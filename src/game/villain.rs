//! The mad scientist (FR23, milestone A): Dr. Nix Nullwell potters around at the
//! top of the road, always a little out of reach, and reacts to what the player
//! does with a speech bubble and an animation. With live lines on (milestone B),
//! `Game` asks Cerebras for each line (`llm::villain_gen`) and hands it in with
//! [`Villain::supply`]; the doctor waits up to `LIVE_WAIT_S` for it and otherwise,
//! and always with no key, says a line from `assets/villain_lines.json`. Nothing
//! here calls the model.
//!
//! They are **cosmetic** (invariant 7): they never touch XP, speed, answers, HP
//! or a battle. They are **not a placed thing** (invariant 3): their position is
//! a function of the camera and the game clock, not of a chunk, so no world
//! generation changes. Every animation runs on the clock `Game` passes in, so a
//! pause freezes them (invariant 8), and the sparks, glow and puffs hold steady
//! under reduce flashing (invariant 9).

use super::dialogue::TYPE_RATE;
use super::gfx::{self, INK};
use super::world::{self, View};
use crate::llm::villain_gen::RawBeat;
use macroquad::prelude::*;
use serde::Deserialize;
use std::cell::OnceCell;
use std::collections::{BTreeMap, VecDeque};
use std::f32::consts::{PI, TAU};

/// How far below the top of the screen the doctor's feet are: a band near the
/// top, tall enough that their head and a speech bubble above it fit, as a share
/// of the window's height and within these limits (px).
const BAND_FRAC: f32 = 0.30;
const BAND_MIN_PX: f32 = 160.0;
const BAND_MAX_PX: f32 = 240.0;
/// The doctor never stands closer than this to the player (px of world `y`), so
/// they can't be reached however the camera lags behind a sprint.
const LEAD_PX: f32 = 150.0;
/// Closer than this to the player and the doctor scurries off: they're being
/// pushed up against the lead they keep.
const SCURRY_GAP_PX: f32 = LEAD_PX + 25.0;
/// How quickly the doctor closes on their target (per second), and when
/// scurrying.
const EASE: f32 = 3.0;
const EASE_SCURRY: f32 = 8.0;
/// How far past the walkable road the doctor wanders, onto the verge (px).
const VERGE_PX: f32 = 70.0;
/// Seconds for the doctor to fade in or out in a soot puff.
const APPEAR_S: f32 = 0.7;
const VANISH_S: f32 = 0.6;
/// Fewest seconds between one bubble and the next (a reaction to something big
/// skips it), and how long a quiet doctor waits before muttering.
const MIN_GAP_S: f32 = 8.0;
const IDLE_S: f32 = 25.0;
/// How long a pending cue stays worth saying. The big ones (priority 6 and up)
/// wait as long as it takes.
const STALE_S: f32 = 6.0;
/// Seconds a bubble stays after it has finished typing, and its fade.
const HOLD_S: f32 = 2.4;
const FADE_S: f32 = 0.4;
/// Seconds an action from a spoken line lasts.
const REACT_S: f32 = 2.8;
/// How long a cue that's ready to be said waits for its live line before the
/// bank's line is said instead (FR23 B). Cerebras usually answers well inside it.
pub const LIVE_WAIT_S: f32 = 1.5;
/// How many recent lines go in the prompt, so the model doesn't repeat itself.
const RECENT: usize = 4;

/// What the doctor is doing. A closed set: the line bank names one for each
/// line, and idle time picks among the first six.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VillainAction {
    /// Strolls across the top of the road, coat flapping.
    Pace,
    /// Bent over the cart with a wrench, sparks flying.
    Tinker,
    /// Pours the flask: a puff of coloured smoke.
    Brew,
    /// Leans back, shoulders shaking: "MWAHAHA!"
    Cackle,
    /// Writes a wrong sum on the chalkboard.
    Scribble,
    /// A telescope pointed at the player.
    Spy,
    /// Frizz up, flask fizzing over: the player got something right.
    Fluster,
    /// Rubs gloves together: the plan is going well.
    Gloat,
    /// Dashes ahead pushing the cart when the player gains on them.
    Scurry,
    /// A soot puff, and they're there.
    Appear,
    /// A soot puff, and they're gone.
    Vanish,
}

impl VillainAction {
    /// The ones idle time chooses between. `Pace` is there twice because a
    /// doctor who is always walking looks busiest.
    const IDLE: [VillainAction; 7] = [
        VillainAction::Pace,
        VillainAction::Tinker,
        VillainAction::Pace,
        VillainAction::Brew,
        VillainAction::Cackle,
        VillainAction::Scribble,
        VillainAction::Spy,
    ];
}

/// How the doctor feels; colours the flask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    Smug,
    Gleeful,
    Flustered,
    Scheming,
    FuriousButFunny,
}

impl Mood {
    /// What the doctor does with a line in this mood when the model didn't say.
    fn usual_action(self) -> VillainAction {
        match self {
            Mood::Smug => VillainAction::Gloat,
            Mood::Gleeful => VillainAction::Cackle,
            Mood::Flustered | Mood::FuriousButFunny => VillainAction::Fluster,
            Mood::Scheming => VillainAction::Tinker,
        }
    }

    /// The colour of the brew in the flask for this mood.
    fn brew(self) -> Color {
        match self {
            Mood::Smug => gfx::LAB_GREEN,
            Mood::Gleeful => Color::new(1.0, 0.45, 0.80, 1.0),
            Mood::Flustered => Color::new(1.0, 0.72, 0.25, 1.0),
            Mood::Scheming => Color::new(0.68, 0.45, 1.0, 1.0),
            Mood::FuriousButFunny => Color::new(1.0, 0.38, 0.30, 1.0),
        }
    }
}

/// Something that happened that the doctor may remark on. Built by `Game` from
/// events that already exist; it carries a little run context for the line's
/// numbers and nothing about the person playing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VillainCue {
    FirstSighting,
    Milestone { meters: u32 },
    FlaskRight,
    FlaskWrong,
    BattleStart,
    BattleWon,
    PlayerHurt,
    Streak { n: u32 },
    LevelUp { level: u32 },
    LowHealth,
    Idle,
}

impl VillainCue {
    /// Every cue's key in `assets/villain_lines.json`, in a fixed order.
    pub const KEYS: [&'static str; 11] = [
        "first_sighting",
        "milestone",
        "flask_right",
        "flask_wrong",
        "battle_start",
        "battle_won",
        "player_hurt",
        "streak",
        "level_up",
        "low_health",
        "idle",
    ];

    pub fn key(self) -> &'static str {
        Self::KEYS[self.index()]
    }

    fn index(self) -> usize {
        match self {
            VillainCue::FirstSighting => 0,
            VillainCue::Milestone { .. } => 1,
            VillainCue::FlaskRight => 2,
            VillainCue::FlaskWrong => 3,
            VillainCue::BattleStart => 4,
            VillainCue::BattleWon => 5,
            VillainCue::PlayerHurt => 6,
            VillainCue::Streak { .. } => 7,
            VillainCue::LevelUp { .. } => 8,
            VillainCue::LowHealth => 9,
            VillainCue::Idle => 10,
        }
    }

    /// How much this deserves to be said over another waiting cue.
    fn priority(self) -> u8 {
        match self {
            VillainCue::FirstSighting => 9,
            VillainCue::LevelUp { .. } => 8,
            VillainCue::Streak { .. } => 7,
            VillainCue::BattleWon => 6,
            VillainCue::Milestone { .. } | VillainCue::LowHealth => 5,
            VillainCue::PlayerHurt | VillainCue::BattleStart => 4,
            VillainCue::FlaskRight | VillainCue::FlaskWrong => 3,
            VillainCue::Idle => 1,
        }
    }

    /// The mood a live line for this cue gets when the model didn't say.
    fn usual_mood(self) -> Mood {
        match self {
            VillainCue::FlaskRight
            | VillainCue::BattleWon
            | VillainCue::Streak { .. }
            | VillainCue::LevelUp { .. }
            | VillainCue::Milestone { .. } => Mood::Flustered,
            VillainCue::FirstSighting | VillainCue::FlaskWrong | VillainCue::PlayerHurt => {
                Mood::Gleeful
            }
            VillainCue::BattleStart => Mood::Scheming,
            VillainCue::LowHealth | VillainCue::Idle => Mood::Smug,
        }
    }

    /// The number this cue is about, for the live prompt.
    pub fn number(self) -> Option<u32> {
        self.placeholder().map(|(_, n)| n)
    }

    /// The placeholder a line may use for this cue, and what it stands for.
    fn placeholder(self) -> Option<(&'static str, u32)> {
        match self {
            VillainCue::Milestone { meters } => Some(("{m}", meters)),
            VillainCue::Streak { n } => Some(("{n}", n)),
            VillainCue::LevelUp { level } => Some(("{level}", level)),
            _ => None,
        }
    }

    /// The cue for a `JEKO_DEV=villain:<name>` event, with made-up numbers.
    pub fn from_dev(name: &str) -> Option<VillainCue> {
        Some(match name {
            "first" | "first_sighting" => VillainCue::FirstSighting,
            "milestone" => VillainCue::Milestone { meters: 100 },
            "right" | "flask_right" => VillainCue::FlaskRight,
            "wrong" | "flask_wrong" => VillainCue::FlaskWrong,
            "battle_start" => VillainCue::BattleStart,
            "battle_won" => VillainCue::BattleWon,
            "hurt" | "player_hurt" => VillainCue::PlayerHurt,
            "streak" => VillainCue::Streak { n: 5 },
            "level_up" | "levelup" => VillainCue::LevelUp { level: 3 },
            "low_health" | "lowhealth" => VillainCue::LowHealth,
            "idle" => VillainCue::Idle,
            _ => return None,
        })
    }
}

// ---------------------------------------------------------------------------
// The line bank

/// One line in `assets/villain_lines.json`.
#[derive(Clone, Debug, Deserialize)]
pub struct Line {
    pub line: String,
    pub mood: Mood,
    pub action: VillainAction,
}

/// The doctor's lines, by cue key.
#[derive(Clone, Debug)]
pub struct Bank(BTreeMap<String, Vec<Line>>);

impl Bank {
    /// How many lines there are in all.
    pub fn len(&self) -> usize {
        self.0.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lines(&self, key: &str) -> &[Line] {
        self.0.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// Fewest and most lines a cue may have, and the longest a line may be (the
/// bubble's budget; FR23's live beats use the same cap).
pub const MIN_LINES: usize = 2;
pub const MAX_LINES: usize = 8;
pub const MAX_LINE_CHARS: usize = 90;

/// The compiled-in bank. Panics at startup on a bad one, like the dialogue bank,
/// and a test runs `check_bank` on the shipped file so that can't happen.
pub fn load_bank() -> Bank {
    check_bank(include_str!("../../assets/villain_lines.json"))
        .expect("assets/villain_lines.json must pass villain::check_bank")
}

/// Live calls a minute unless `JEKO_VILLAIN_RPM` says otherwise.
pub const DEFAULT_RPM: usize = 6;

/// Whether the doctor's lines are asked of the model (FR23 B), from the env.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveConfig {
    /// `JEKO_VILLAIN=off` keeps every line in the bank.
    pub enabled: bool,
    pub rpm: usize,
}

impl LiveConfig {
    pub fn from_env() -> Self {
        let var = |k| std::env::var(k).ok();
        Self::parse(
            var("JEKO_VILLAIN").as_deref(),
            var("JEKO_VILLAIN_RPM").as_deref(),
        )
    }

    fn parse(switch: Option<&str>, rpm: Option<&str>) -> Self {
        Self {
            enabled: !matches!(
                switch.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
                Some("off" | "0" | "false" | "no")
            ),
            rpm: rpm
                .and_then(|r| r.trim().parse().ok())
                .unwrap_or(DEFAULT_RPM),
        }
    }
}

/// The game's half of checking a live beat (FR23 B): `llm::villain_gen::parse_reply`
/// has already trimmed and cut it. Refused (`None`, so the bank's line is said)
/// when the line hits the blocklist or still has a `{placeholder}`. An unknown
/// mood becomes the cue's usual one, and an unknown action (or `appear` /
/// `vanish`, which only the game picks) becomes the mood's usual one. Pure.
pub fn beat_line(beat: &RawBeat, cue: VillainCue) -> Option<Line> {
    let line = beat.line.trim();
    if line.is_empty()
        || line.contains(['{', '}'])
        || line.chars().count() > MAX_LINE_CHARS
        || crate::content::vet::vet_text(line).is_err()
    {
        return None;
    }
    let named = |s: &Option<String>| s.as_ref().map(|s| serde_json::Value::String(s.clone()));
    let mood = named(&beat.mood)
        .and_then(|v| serde_json::from_value::<Mood>(v).ok())
        .unwrap_or_else(|| cue.usual_mood());
    let action = named(&beat.action)
        .and_then(|v| serde_json::from_value::<VillainAction>(v).ok())
        .filter(|a| !matches!(a, VillainAction::Appear | VillainAction::Vanish))
        .unwrap_or_else(|| mood.usual_action());
    Some(Line {
        line: line.to_string(),
        mood,
        action,
    })
}

/// Parses and checks a bank of lines: an object with one array per cue key
/// (every key present, none unknown), each with 2 to 8 lines of at most 90
/// characters (counted with the largest number a placeholder could hold), no
/// word on the blocklist (`content::vet`), a `{m}`, `{n}` or `{level}`
/// placeholder only on the cue that has it, and no `appear` or `vanish`
/// action, which only the game chooses. The first problem found is the error.
/// `validate_content` runs it on the shipped file.
pub fn check_bank(json: &str) -> Result<Bank, String> {
    let map: BTreeMap<String, Vec<Line>> =
        serde_json::from_str(json).map_err(|e| format!("not a bank of cue lines: {e}"))?;
    if let Some(key) = map.keys().find(|k| !VillainCue::KEYS.contains(&k.as_str())) {
        return Err(format!("unknown cue {key:?}"));
    }
    for key in VillainCue::KEYS {
        let lines = map
            .get(key)
            .ok_or_else(|| format!("cue {key:?} has no lines"))?;
        if !(MIN_LINES..=MAX_LINES).contains(&lines.len()) {
            return Err(format!(
                "{key} has {} lines, want {MIN_LINES} to {MAX_LINES}",
                lines.len()
            ));
        }
        // The one placeholder this cue's lines may use.
        let allowed = match key {
            "milestone" => Some("{m}"),
            "streak" => Some("{n}"),
            "level_up" => Some("{level}"),
            _ => None,
        };
        for l in lines {
            if l.line.trim().is_empty() {
                return Err(format!("{key} has an empty line"));
            }
            for p in ["{m}", "{n}", "{level}"] {
                if l.line.contains(p) && allowed != Some(p) {
                    return Err(format!("{key}: {:?} can't use {p}", l.line));
                }
            }
            let widest = match allowed {
                Some(p) => l.line.replace(p, "99999"),
                None => l.line.clone(),
            };
            if widest.chars().count() > MAX_LINE_CHARS {
                return Err(format!(
                    "{key}: {:?} is over {MAX_LINE_CHARS} characters",
                    l.line
                ));
            }
            if let Err(word) = crate::content::vet::vet_text(&l.line) {
                return Err(format!(
                    "{key}: {:?} uses the blocked word {word:?}",
                    l.line
                ));
            }
            if matches!(l.action, VillainAction::Appear | VillainAction::Vanish) {
                return Err(format!("{key}: a line can't use {:?}", l.action));
            }
        }
    }
    Ok(Bank(map))
}

// ---------------------------------------------------------------------------
// Where they stand

/// Where the doctor wants to stand: a band near the top of the screen, never
/// within `LEAD_PX` of the player, wandering along the road and the verge beside
/// it. `view_h` is the window's height and `wander` the doctor's own slow clock.
/// Pure, so it can be tested.
pub fn target_pos(cam_y: f32, view_h: f32, player: Vec2, wander: f32) -> Vec2 {
    let band = (view_h * BAND_FRAC).clamp(BAND_MIN_PX, BAND_MAX_PX);
    let y = (cam_y + band).min(player.y - LEAD_PX);
    let (lo, hi) = world::walkable_x(y);
    let mid = (lo + hi) * 0.5;
    let half = (hi - lo) * 0.5 + VERGE_PX;
    let x = mid + half * (0.62 * (wander * 0.21).sin() + 0.38 * (wander * 0.47 + 1.3).sin());
    vec2(x, y)
}

/// A reading of the run, for [`Villain::observe`] to turn into cues.
#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    pub level: u32,
    pub streak: u32,
    pub meters: u32,
    pub health: u32,
}

struct Bubble {
    lines: Vec<String>,
    chars: usize,
    /// Seconds since it opened.
    t: f32,
}

impl Bubble {
    fn life(&self) -> f32 {
        self.chars as f32 / TYPE_RATE + HOLD_S + FADE_S
    }

    fn alpha(&self) -> f32 {
        ((self.life() - self.t) / FADE_S).clamp(0.0, 1.0)
    }
}

/// The mad scientist.
pub struct Villain {
    bank: Bank,
    /// Where their feet are, in world coordinates.
    pos: Vec2,
    /// +1 facing right, -1 left.
    face: f32,
    action: VillainAction,
    /// Seconds in the current action, and when the next one is picked.
    action_t: f32,
    action_len: f32,
    mood: Mood,
    /// 0 is gone and 1 fully there: ramps with `Appear` and `Vanish`.
    presence: f32,
    /// They're out on the road (the player is past the gate).
    here: bool,
    /// They've had their first sighting this run.
    met: bool,
    bubble: Option<Bubble>,
    /// A cue waiting to be said, and how long it has waited.
    pending: Option<(VillainCue, f32)>,
    since_bubble: f32,
    /// Times each cue has been said, so its lines rotate.
    said: [u32; VillainCue::KEYS.len()],
    /// Picks made, to vary what they do next without a random generator.
    picks: u32,
    /// Their own slow clock for wandering; runs only while they're on the move.
    wander: f32,
    /// Distance covered, which turns the cart's wheels and drives the stride.
    travel: f32,
    /// The wrong sum on the chalkboard, and which one.
    sum: usize,
    prev: Snapshot,
    /// Live lines are on (FR23 B): a ready cue waits briefly for one.
    live: bool,
    /// The waiting cue a live line has been asked for, and the line once it's in.
    asked: Option<VillainCue>,
    ready: Option<Line>,
    /// How long the waiting cue has been ready to say while its line is out.
    held: f32,
    /// The last few lines said, for the prompt.
    recent: VecDeque<String>,
    /// Lines said this run that came from the model, and from the bank.
    pub live_said: u32,
    pub bank_said: u32,
    canvas: OnceCell<RenderTarget>,
}

impl Default for Villain {
    fn default() -> Self {
        Self::new()
    }
}

impl Villain {
    pub fn new() -> Self {
        Self::with_bank(load_bank())
    }

    fn with_bank(bank: Bank) -> Self {
        Self {
            bank,
            pos: Vec2::ZERO,
            face: 1.0,
            action: VillainAction::Vanish,
            action_t: 0.0,
            action_len: f32::MAX,
            mood: Mood::Smug,
            presence: 0.0,
            here: false,
            met: false,
            bubble: None,
            pending: None,
            since_bubble: 0.0,
            said: [0; VillainCue::KEYS.len()],
            picks: 0,
            wander: 0.0,
            travel: 0.0,
            sum: 0,
            prev: Snapshot::default(),
            live: false,
            asked: None,
            ready: None,
            held: 0.0,
            recent: VecDeque::new(),
            live_said: 0,
            bank_said: 0,
            canvas: OnceCell::new(),
        }
    }

    /// Back to before the first sighting, for a new run. Keeps the bank and the
    /// canvas.
    pub fn reset(&mut self) {
        let bank = std::mem::replace(&mut self.bank, Bank(BTreeMap::new()));
        let canvas = std::mem::take(&mut self.canvas);
        let live = self.live;
        *self = Self {
            canvas,
            live,
            ..Self::with_bank(bank)
        };
    }

    /// Turns live lines on or off (FR23 B). Off, every line is the bank's.
    pub fn set_live(&mut self, on: bool) {
        self.live = on;
        if !on {
            self.asked = None;
            self.ready = None;
        }
    }

    /// The waiting cue to ask the model about, if live lines are on, the doctor
    /// is out and it hasn't been asked for yet.
    pub fn wants_line(&self) -> Option<VillainCue> {
        let (cue, _) = self.pending?;
        (self.live && self.here && self.asked != Some(cue)).then_some(cue)
    }

    /// `Game` has asked the model for `cue`'s line.
    pub fn mark_asked(&mut self, cue: VillainCue) {
        self.asked = Some(cue);
        self.ready = None;
        self.held = 0.0;
    }

    /// A live line for `cue` has arrived (`None`: the call failed or the line was
    /// refused, so the bank's is said without waiting any longer). A line for a
    /// cue that's no longer waiting is dropped.
    pub fn supply(&mut self, cue: VillainCue, line: Option<Line>) {
        if self.asked != Some(cue) {
            return;
        }
        match line {
            Some(l) => self.ready = Some(l),
            None => self.asked = None,
        }
    }

    /// The last few lines said, oldest first, for the live prompt.
    pub fn recent_lines(&self) -> Vec<String> {
        self.recent.iter().cloned().collect()
    }

    /// Whether any of them is on screen to draw.
    pub fn visible(&self) -> bool {
        self.presence > 0.0
    }

    pub fn action(&self) -> VillainAction {
        self.action
    }

    pub fn is_here(&self) -> bool {
        self.here
    }

    /// The line on screen, if any.
    pub fn saying(&self) -> Option<String> {
        self.bubble.as_ref().map(|b| b.lines.join(" "))
    }

    /// The ground `y` they sort by in the depth pass.
    pub fn ground_y(&self) -> f32 {
        self.pos.y
    }

    fn set_action(&mut self, action: VillainAction, len: f32) {
        self.action = action;
        self.action_t = 0.0;
        self.action_len = len;
        if action == VillainAction::Scribble {
            self.sum += 1;
        }
    }

    /// Something happened. The doctor remarks on it when they're next free to,
    /// unless it's already stale or something bigger came along. Dropped while
    /// they're away (the hub stays calm).
    pub fn cue(&mut self, cue: VillainCue) {
        if !self.here {
            return;
        }
        let better = match self.pending {
            Some((p, _)) => cue.priority() >= p.priority(),
            None => true,
        };
        if better {
            self.pending = Some((cue, 0.0));
        }
    }

    /// Says `cue` now, whatever else is going on, for the dev hook.
    pub fn cue_now(&mut self, cue: VillainCue) {
        if self.here {
            self.speak(cue);
        }
    }

    /// Turns changes in the run into cues: a level, a streak of 5 or each 10, each
    /// 100 m, and hearts running low. Called every unpaused frame. `Game` fires
    /// the ones that belong to a single event (an answer, a hit, a battle).
    pub fn observe(&mut self, now: Snapshot) {
        let prev = self.prev;
        self.prev = now;
        if now.level > prev.level && prev.level > 0 {
            self.cue(VillainCue::LevelUp { level: now.level });
        }
        if now.streak > prev.streak && (now.streak == 5 || now.streak.is_multiple_of(10)) {
            self.cue(VillainCue::Streak { n: now.streak });
        }
        if now.meters / 100 > prev.meters / 100 {
            self.cue(VillainCue::Milestone {
                meters: now.meters / 100 * 100,
            });
        }
        if now.health > 0 && now.health <= 2 && prev.health > 2 {
            self.cue(VillainCue::LowHealth);
        }
    }

    /// Advances the doctor by `dt` seconds of game time. `time` is the game
    /// clock, `cam_y` the world `y` at the top of the screen and `view_h` the
    /// window's height. Called only on the
    /// road, so a battle, a dialogue and a pause leave them frozen.
    pub fn update(&mut self, dt: f32, cam_y: f32, view_h: f32, player: Vec2) {
        // Past the gate and not yet at the castle: near it, they've gone inside (FR27).
        let want = player.y <= world::ROAD_START_Y && !world::near_the_castle(player.y);
        if want && !self.here {
            self.here = true;
            self.pos = target_pos(cam_y, view_h, player, self.wander);
            self.set_action(VillainAction::Appear, APPEAR_S);
            if !self.met {
                self.met = true;
                self.pending = Some((VillainCue::FirstSighting, 0.0));
            }
        } else if !want && self.here {
            self.here = false;
            self.pending = None;
            self.bubble = None;
            self.set_action(VillainAction::Vanish, VANISH_S);
        }

        let step = dt / if self.here { APPEAR_S } else { VANISH_S };
        self.presence = (self.presence + if self.here { step } else { -step }).clamp(0.0, 1.0);
        self.action_t += dt;
        if !self.here {
            return;
        }

        self.since_bubble += dt;
        if let Some(b) = &mut self.bubble {
            b.t += dt;
            if b.t >= b.life() {
                self.bubble = None;
            }
        }
        if let Some((cue, age)) = &mut self.pending {
            *age += dt;
            if *age > STALE_S && cue.priority() < 6 {
                self.pending = None;
            }
        }

        // Pick what to do next: scurry if the player is gaining, say the waiting
        // cue if it's time, or else drift into something from the idle list.
        let gap = player.y - self.pos.y;
        let appearing = self.action == VillainAction::Appear && self.action_t < self.action_len;
        if gap < SCURRY_GAP_PX && !appearing {
            if self.action != VillainAction::Scurry {
                self.set_action(VillainAction::Scurry, 1.2);
            } else {
                self.action_t = 0.0;
            }
        } else if let Some((cue, _)) = self.pending {
            let free = self.since_bubble >= MIN_GAP_S || cue.priority() >= 9;
            if free && !appearing {
                // A live line that's on its way gets a moment to arrive.
                let waiting = self.live
                    && self.asked == Some(cue)
                    && self.ready.is_none()
                    && self.held < LIVE_WAIT_S;
                if waiting {
                    self.held += dt;
                } else {
                    self.pending = None;
                    let line = if self.asked == Some(cue) {
                        self.ready.take()
                    } else {
                        None
                    };
                    self.asked = None;
                    self.speak_line(cue, line);
                }
            }
        } else if self.since_bubble >= IDLE_S && !appearing {
            if self.live {
                // Queued like any cue, so its live line can be asked for.
                self.pending = Some((VillainCue::Idle, 0.0));
            } else {
                self.speak(VillainCue::Idle);
            }
        }
        if self.action_t >= self.action_len {
            self.next_action();
        }

        // Ease toward the target. They only wander along while on the move.
        let moving = matches!(self.action, VillainAction::Pace | VillainAction::Scurry);
        if moving {
            self.wander += dt
                * if self.action == VillainAction::Scurry {
                    2.4
                } else {
                    1.0
                };
        }
        let goal = target_pos(cam_y, view_h, player, self.wander);
        let k = 1.0
            - (-dt
                * if self.action == VillainAction::Scurry {
                    EASE_SCURRY
                } else {
                    EASE
                })
            .exp();
        let before = self.pos;
        self.pos += (goal - self.pos) * k;
        let moved = self.pos - before;
        if moved.x.abs() > 0.05 {
            self.face = moved.x.signum();
        }
        self.travel += moved.length();
    }

    fn next_action(&mut self) {
        self.picks += 1;
        let n = VillainAction::IDLE.len();
        let r = gfx::hash01(self.picks as u64 * 7919 + 17);
        let mut i = (r * n as f32) as usize % n;
        if VillainAction::IDLE[i] == self.action {
            i = (i + 1) % n;
        }
        let len = 3.5 + 3.0 * gfx::hash01(self.picks as u64 * 104_729 + 3);
        self.set_action(VillainAction::IDLE[i], len);
    }

    /// Opens a bubble for `cue` with a line from the bank and acts it out.
    fn speak(&mut self, cue: VillainCue) {
        self.speak_line(cue, None);
    }

    /// Opens a bubble for `cue` and acts out `live`, or a line from the bank.
    fn speak_line(&mut self, cue: VillainCue, live: Option<Line>) {
        let (text, mood, action) = match live {
            Some(l) => {
                self.live_said += 1;
                (l.line, l.mood, l.action)
            }
            None => {
                let lines = self.bank.lines(cue.key());
                if lines.is_empty() {
                    return;
                }
                let k = cue.index();
                let start = (gfx::hash01(k as u64 + 5) * lines.len() as f32) as usize;
                let pick = &lines[(start + self.said[k] as usize) % lines.len()];
                self.said[k] += 1;
                self.bank_said += 1;
                let mut text = pick.line.clone();
                if let Some((p, n)) = cue.placeholder() {
                    text = text.replace(p, &n.to_string());
                }
                (text, pick.mood, pick.action)
            }
        };
        self.recent.push_back(text.clone());
        if self.recent.len() > RECENT {
            self.recent.pop_front();
        }
        self.mood = mood;
        self.bubble = Some(Bubble {
            chars: text.chars().count(),
            lines: gfx::wrap_text(&text, 34),
            t: 0.0,
        });
        self.since_bubble = 0.0;
        self.set_action(action, REACT_S);
    }

    // -----------------------------------------------------------------------
    // Drawing

    /// Draws the doctor with their feet on the ground at `pos` (`lift` is the
    /// height of whatever they stand on). One piece through a 2x canvas, so the
    /// soot puff can fade them in and out without seams.
    pub fn draw(&self, view: &View, lift: f32, time: f32, reduce_flashing: bool) {
        if self.presence <= 0.0 {
            return;
        }
        let ground = view.to_screen(self.pos) - vec2(0.0, lift);
        // A little haze (towards the night colour) so they read as far up the road.
        let haze = gfx::mix(WHITE, gfx::NIGHT, 0.14);
        let tint = Color::new(haze.r, haze.g, haze.b, self.presence);
        gfx::ground_shadow(ground, 24.0 * self.presence, 7.0);

        let canvas = self.canvas.get_or_init(|| {
            let rt = render_target((CW * RES) as u32, (CH * RES) as u32);
            rt.texture.set_filter(FilterMode::Linear);
            rt
        });
        let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, CW, CH));
        cam.render_target = Some(canvas.clone());
        set_camera(&cam);
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
        self.draw_sprite(vec2(FOOT_X, FOOT_Y), time, reduce_flashing);
        set_default_camera();
        draw_texture_ex(
            &canvas.texture,
            ground.x - FOOT_X,
            ground.y - FOOT_Y,
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(CW, CH)),
                // Render targets come back bottom-up.
                flip_y: true,
                ..Default::default()
            },
        );

        // The soot puff of an appearance or a vanishing, over the top.
        let u = match self.action {
            VillainAction::Appear => (self.action_t / APPEAR_S).clamp(0.0, 1.0),
            VillainAction::Vanish => (self.action_t / VANISH_S).clamp(0.0, 1.0),
            _ => return,
        };
        let centre = ground - vec2(0.0, 36.0);
        for i in 0..9 {
            let a = i as f32 / 9.0 * TAU + gfx::hash01(i as u64) * 0.8;
            let r = 10.0 + 36.0 * u * (0.6 + gfx::hash01(i as u64 + 40));
            let p = centre + vec2(a.cos() * r, a.sin() * r * 0.7 - 14.0 * u);
            draw_circle(
                p.x,
                p.y,
                9.0 + 9.0 * u,
                Color::new(0.20, 0.18, 0.22, 0.55 * (1.0 - u)),
            );
        }
    }

    /// The speech bubble over their head, in screen space. Drawn after the
    /// night's darkening so it's always readable.
    pub fn draw_bubble(&self, view: &View, lift: f32) {
        let Some(b) = &self.bubble else {
            return;
        };
        if self.presence < 1.0 {
            return;
        }
        let head = view.to_screen(self.pos) - vec2(0.0, lift + 98.0);
        let shown = (b.t * TYPE_RATE) as usize;
        gfx::speech_bubble(head, &b.lines, shown, b.alpha());
    }

    /// The doctor and their cart, drawn into the canvas with the feet at `foot`.
    fn draw_sprite(&self, foot: Vec2, time: f32, rf: bool) {
        let p = pose(self.action, self.action_t, time, self.travel);
        let f = self.face;
        let at = |x: f32, y: f32| vec2(foot.x + x * f, foot.y + y);
        let ink = INK;
        let coat = Color::new(0.949, 0.933, 0.875, 1.0);
        let coat_shade = gfx::shade(coat, 0.82);
        let brass = Color::new(0.784, 0.608, 0.235, 1.0);
        let glove = Color::new(0.557, 0.357, 0.839, 1.0);
        let boot = Color::new(0.17, 0.12, 0.20, 1.0);
        let skin = Color::new(0.95, 0.80, 0.66, 1.0);
        let frizz = Color::new(0.97, 0.97, 1.0, 1.0);
        let lens = gfx::LAB_GREEN;
        let brew = self.mood.brew();

        // --- the cart, in front of them (the way they're facing) ---
        let cx = 64.0;
        // Wheels turn with the distance covered.
        for wx in [cx - 17.0, cx + 17.0] {
            let c = at(wx, -9.0);
            draw_circle(c.x, c.y, 10.0, ink);
            draw_circle(c.x, c.y, 8.2, Color::new(0.45, 0.32, 0.22, 1.0));
            for s in 0..4 {
                let a = p.wheel + s as f32 * PI / 2.0;
                gfx::line(
                    c,
                    c + vec2(a.cos(), a.sin()) * 7.4,
                    1.6,
                    gfx::shade(brass, 0.8),
                );
            }
            draw_circle(c.x, c.y, 2.2, brass);
        }
        // The bench.
        let bench = [
            at(cx - 27.0, -17.0),
            at(cx + 27.0, -17.0),
            at(cx + 27.0, -29.0),
            at(cx - 27.0, -29.0),
        ];
        gfx::quad(
            bench[0] + vec2(-1.5, 1.5),
            bench[1] + vec2(1.5, 1.5),
            bench[2] + vec2(1.5, -1.5),
            bench[3] + vec2(-1.5, -1.5),
            ink,
        );
        gfx::quad(
            bench[0],
            bench[1],
            bench[2],
            bench[3],
            Color::new(0.56, 0.40, 0.27, 1.0),
        );
        let top = at(cx, -29.0);
        draw_rectangle(top.x - 27.0, top.y - 2.0, 54.0, 3.0, brass);
        // The crank on the side turns while they tinker.
        let crank = at(cx + 29.0, -22.0);
        draw_circle(crank.x, crank.y, 3.0, brass);
        let ca = p.crank;
        gfx::line(crank, crank + vec2(ca.cos(), ca.sin()) * 7.0, 2.0, ink);
        // The Tesla coil: a pole, a ball, and sparks.
        let pole = at(cx + 15.0, -29.0);
        draw_rectangle(pole.x - 1.6, pole.y - 26.0, 3.2, 26.0, ink);
        let ball = pole - vec2(0.0, 30.0);
        draw_circle(ball.x, ball.y, 6.4, ink);
        draw_circle(ball.x, ball.y, 5.0, Color::new(0.75, 0.80, 0.90, 1.0));
        draw_circle(ball.x - 1.5, ball.y - 1.5, 1.6, WHITE);
        if p.sparks > 0.0 {
            // Steady glow when flashing is reduced; otherwise the sparks jump.
            let glow_a = 0.25 + 0.25 * p.sparks;
            gfx::glow(ball, 16.0, 16.0, gfx::with_alpha(lens, glow_a));
            if !rf {
                for s in 0..3 {
                    let k = (time * 14.0 + s as f32 * 2.3).floor();
                    let a = gfx::hash01(k as u64 * 3 + s as u64) * TAU;
                    let len = 8.0 + 8.0 * gfx::hash01(k as u64 + 99);
                    let mid = ball + vec2(a.cos(), a.sin()) * len * 0.5 + vec2(2.0 * a.sin(), 0.0);
                    let end = ball + vec2(a.cos(), a.sin()) * len;
                    gfx::polyline(
                        &[ball, mid, end],
                        1.3,
                        gfx::with_alpha(lens, 0.9 * p.sparks),
                    );
                }
            }
        }
        // The chalkboard, on its own stand at the back of the bench.
        let bd = at(cx - 15.0, -29.0);
        draw_rectangle(bd.x - 1.0, bd.y - 14.0, 2.0, 14.0, ink);
        let (bw, bh) = (30.0, 22.0);
        let bx = at(cx - 15.0, 0.0).x - bw * 0.5;
        let by = bd.y - 14.0 - bh;
        draw_rectangle(
            bx - 2.0,
            by - 2.0,
            bw + 4.0,
            bh + 4.0,
            Color::new(0.56, 0.40, 0.27, 1.0),
        );
        draw_rectangle(bx, by, bw, bh, Color::new(0.10, 0.20, 0.16, 1.0));
        draw_text(
            "2+2=5",
            bx + 3.0,
            by + 9.0,
            12.0,
            Color::new(0.95, 0.95, 0.9, 0.95),
        );
        if p.scribble > 0.0 {
            let sum = SUMS[self.sum % SUMS.len()];
            let n = ((sum.len() as f32 * p.scribble).ceil() as usize).min(sum.len());
            draw_text(
                &sum[..n],
                bx + 3.0,
                by + 19.0,
                12.0,
                Color::new(0.95, 0.95, 0.9, 0.95),
            );
        }

        // --- the doctor ---
        let sway = p.lean;
        let bob = p.bob;
        let shake = p.shake;
        // Boots, then the coat's tails behind the legs.
        let stride = p.stride;
        for (side, ph) in [(-1.0, 0.0), (1.0, PI)] {
            let lift = (stride + ph).sin().max(0.0) * 3.0 * p.walking;
            let sx = side * 6.0 + (stride + ph).cos() * 3.5 * p.walking;
            let c = at(sx, -2.5 - lift);
            gfx::ellipse(vec2(c.x, c.y), 8.0, 4.6, ink);
            gfx::ellipse(vec2(c.x, c.y), 6.8, 3.6, boot);
            // The leg: a stalk of coat-coloured trouser.
            gfx::line(at(side * 5.0, -14.0 + bob), c - vec2(0.0, 2.0), 4.4, ink);
            gfx::line(
                at(side * 5.0, -14.0 + bob),
                c - vec2(0.0, 2.0),
                2.8,
                Color::new(0.38, 0.36, 0.5, 1.0),
            );
        }
        // The back arm, behind the coat.
        let sh_back = at(-4.0 + sway * 0.4, -50.0 + bob);
        let hand_back = arm_hand(sh_back, p.arm_b, f, 17.0);
        gfx::line(sh_back, hand_back, 6.0, ink);
        gfx::line(sh_back, hand_back, 4.2, coat_shade);
        draw_circle(hand_back.x, hand_back.y, 4.3, ink);
        draw_circle(hand_back.x, hand_back.y, 3.2, glove);

        // The coat: a long trapezoid with flapping tails.
        let flap = (time * 9.0 + self.travel * 0.1).sin() * p.flap * 5.0;
        let s_l = at(-10.0 + sway * 0.5, -52.0 + bob);
        let s_r = at(10.0 + sway * 0.5, -52.0 + bob);
        let h_l = at(-17.0 - flap * 0.6, -12.0);
        let h_r = at(17.0 + flap, -12.0);
        let out = |a: Vec2, b: Vec2, c: Vec2, d: Vec2| {
            gfx::quad(
                a + vec2(-1.8, -1.8),
                b + vec2(1.8, -1.8),
                c + vec2(1.8, 1.8),
                d + vec2(-1.8, 1.8),
                ink,
            )
        };
        out(s_l, s_r, h_r, h_l);
        gfx::quad(s_l, s_r, h_r, h_l, coat);
        // A shaded edge, a split in the tails, scorch marks and a patch.
        gfx::quad(
            s_r,
            s_r + vec2(-4.0 * f, 0.0),
            h_r + vec2(-5.0 * f, 0.0),
            h_r,
            coat_shade,
        );
        gfx::line(
            at(0.0 + sway * 0.25, -34.0 + bob * 0.5),
            at(0.0 - flap * 0.2, -12.0),
            1.4,
            gfx::with_alpha(ink, 0.5),
        );
        let scorch = Color::new(0.30, 0.25, 0.25, 0.55);
        let m = at(-8.0, -22.0);
        gfx::ellipse_rot(vec2(m.x, m.y), 4.0, 3.0, 20.0, scorch);
        let m = at(9.0, -17.0);
        gfx::ellipse(vec2(m.x, m.y), 3.0, 2.4, scorch);
        let patch = at(-4.0, -30.0);
        draw_rectangle(
            patch.x - 3.0,
            patch.y - 3.0,
            6.0,
            6.0,
            Color::new(0.64, 0.74, 0.88, 1.0),
        );
        draw_rectangle_lines(
            patch.x - 3.0,
            patch.y - 3.0,
            6.0,
            6.0,
            1.0,
            gfx::with_alpha(ink, 0.7),
        );
        // The breast pocket with test tubes and pencils.
        let pk = at(5.0 + sway * 0.4, -44.0 + bob);
        for (dx, col) in [
            (-2.5, lens),
            (0.5, Color::new(1.0, 0.7, 0.3, 1.0)),
            (3.0, brass),
        ] {
            gfx::line(pk + vec2(dx * f, 1.0), pk + vec2(dx * f, -5.0), 1.8, col);
        }
        draw_rectangle(pk.x - 4.5, pk.y, 9.0, 6.0, coat_shade);
        draw_rectangle_lines(pk.x - 4.5, pk.y, 9.0, 6.0, 1.0, gfx::with_alpha(ink, 0.7));
        // Soot on a fluster.
        if p.soot > 0.0 {
            for s in 0..4 {
                let a = gfx::hash01(s as u64 + 700) * TAU;
                let q = at(a.cos() * 10.0, -46.0 + a.sin() * 12.0 - 8.0 * p.soot);
                draw_circle(
                    q.x,
                    q.y,
                    4.5 * (1.0 - 0.4 * p.soot),
                    Color::new(0.15, 0.14, 0.17, 0.5 * p.soot),
                );
            }
        }

        // The head, the bow tie and the neck.
        let jig = vec2(shake, 0.0);
        let hc = at(2.0 + sway, -72.0 + bob) + jig;
        let tie = at(1.0 + sway * 0.6, -53.0 + bob);
        draw_triangle(tie, tie + vec2(-8.0, -4.0), tie + vec2(-8.0, 5.0), ink);
        draw_triangle(tie, tie + vec2(8.0, -5.0), tie + vec2(8.0, 4.0), ink);
        draw_triangle(
            tie,
            tie + vec2(-6.5, -3.0),
            tie + vec2(-6.5, 3.5),
            Color::new(0.74, 0.22, 0.34, 1.0),
        );
        draw_triangle(
            tie,
            tie + vec2(6.5, -3.6),
            tie + vec2(6.5, 2.8),
            Color::new(0.74, 0.22, 0.34, 1.0),
        );
        // Frizz puffs behind the dome: they grow and spark when flustered.
        let puff = 7.0 + 4.0 * p.frizz;
        for side in [-1.0, 1.0] {
            for (dx, dy, r) in [
                (0.0, 0.0, puff),
                (side * 2.0, -puff * 0.9, puff * 0.8),
                (side * 5.0, puff * 0.6, puff * 0.7),
            ] {
                let c = hc + vec2(side * 14.0 + dx, -2.0 + dy);
                draw_circle(c.x, c.y, r + 1.4, ink);
                draw_circle(c.x, c.y, r, frizz);
            }
        }
        draw_circle(hc.x, hc.y, 15.2, ink);
        draw_circle(hc.x, hc.y, 13.6, skin);
        // The bald dome's shine and the one cowlick.
        gfx::ellipse_rot(
            vec2(hc.x - 5.0 * f, hc.y - 8.0),
            4.0,
            2.2,
            -20.0 * f,
            gfx::with_alpha(WHITE, 0.45),
        );
        gfx::polyline(
            &[
                hc + vec2(0.0, -13.0),
                hc + vec2(3.0 * f, -19.0),
                hc + vec2(0.0, -24.0),
            ],
            2.0,
            frizz,
        );
        // The long nose.
        let nose = hc + vec2(10.0 * f, 3.0);
        gfx::ellipse_rot(vec2(nose.x, nose.y), 6.5, 3.8, 10.0 * f, ink);
        gfx::ellipse_rot(
            vec2(nose.x, nose.y),
            5.4,
            2.9,
            10.0 * f,
            gfx::shade(skin, 0.92),
        );
        // Goggles: brass rims, glowing green lenses (steady under reduce flashing),
        // and a strap round the head.
        let pulse = if rf {
            0.85
        } else {
            0.8 + 0.2 * (time * 2.2).sin()
        };
        gfx::line(hc + vec2(-13.0, -4.0), hc + vec2(13.0, -4.0), 2.2, ink);
        for ex in [-5.0, 6.5] {
            let c = hc + vec2(ex * f, -4.0);
            draw_circle(c.x, c.y, 6.6, ink);
            draw_circle(c.x, c.y, 5.6, brass);
            draw_circle(c.x, c.y, 4.2, gfx::with_alpha(lens, pulse));
            draw_circle(c.x - 1.3, c.y - 1.3, 1.2, gfx::with_alpha(WHITE, 0.85));
        }
        gfx::glow(
            hc + vec2(0.0, -4.0),
            15.0,
            8.0,
            gfx::with_alpha(lens, 0.22 * pulse),
        );
        // Bushy brows (they waggle), a toothy grin.
        for ex in [-5.0, 6.5] {
            let c = hc + vec2(ex * f, -11.0 - p.brow);
            gfx::line(
                c + vec2(-4.0, 1.0 - p.brow * 0.3),
                c + vec2(4.0, -1.0 + p.brow * 0.3),
                2.6,
                Color::new(0.85, 0.85, 0.92, 1.0),
            );
        }
        let grin = hc + vec2(3.0 * f, 8.0);
        gfx::ellipse(
            vec2(grin.x, grin.y),
            7.0,
            3.4 + 1.0 * p.brow.abs().min(1.0),
            ink,
        );
        gfx::ellipse(vec2(grin.x, grin.y - 0.6), 6.0, 2.2, WHITE);
        draw_line(
            grin.x - 5.0,
            grin.y - 0.6,
            grin.x + 5.0,
            grin.y - 0.6,
            0.8,
            gfx::with_alpha(ink, 0.6),
        );

        // The front arm and the flask (or the telescope).
        let sh = at(8.0 + sway * 0.5, -50.0 + bob);
        let hand = arm_hand(sh, p.arm_a, f, 17.0);
        if p.spy > 0.0 {
            // A telescope up to the eye, pointed down and forward at the player.
            let eye = hc + vec2(7.0 * f, -3.0);
            let tip = eye + vec2(15.0 * f, 11.0);
            gfx::line(sh, eye + vec2(2.0 * f, 5.0), 6.0, ink);
            gfx::line(sh, eye + vec2(2.0 * f, 5.0), 4.2, coat_shade);
            gfx::line(eye, tip, 7.0, ink);
            gfx::line(eye, tip, 5.0, brass);
            gfx::line(tip - vec2(3.0 * f, 2.0), tip, 7.5, gfx::shade(brass, 0.7));
            draw_circle(eye.x + 2.0 * f, eye.y + 5.0, 3.6, glove);
        } else {
            gfx::line(sh, hand, 6.0, ink);
            gfx::line(sh, hand, 4.2, coat_shade);
            // The flask in the glove, its brew the colour of the mood. It fizzes
            // over on a fluster.
            gfx::potion_flask(
                hand + vec2(0.0, 6.0),
                0.85,
                brew,
                1.0,
                time * (1.0 + 1.5 * p.fizz),
                1.0,
            );
            draw_circle(hand.x, hand.y, 4.4, ink);
            draw_circle(hand.x, hand.y, 3.3, glove);
            let mouth = hand + vec2(0.0, -17.0);
            if p.fizz > 0.0 {
                for s in 0..5 {
                    let u = (time * 1.3 + s as f32 * 0.21).fract();
                    let q = mouth
                        + vec2(
                            (s as f32 - 2.0) * 2.4 + (u * 9.0 + s as f32).sin() * 2.0,
                            -u * 14.0,
                        );
                    draw_circle(
                        q.x,
                        q.y,
                        2.4 * (1.0 - u * 0.5),
                        gfx::with_alpha(gfx::mix(brew, WHITE, 0.4), 0.8 * (1.0 - u)),
                    );
                }
            }
            // Brewing: a thin stream onto the bench, and a puff of coloured smoke.
            if p.pour > 0.0 {
                let to = at(cx - 4.0, -30.0);
                let from = mouth + vec2(3.0 * f, 2.0);
                gfx::line(
                    from,
                    vec2(to.x, to.y),
                    1.8,
                    gfx::with_alpha(brew, 0.85 * p.pour),
                );
                for s in 0..4 {
                    let u = (time * 0.8 + s as f32 * 0.27).fract();
                    let q = vec2(to.x, to.y)
                        + vec2((s as f32 - 1.5) * 4.0 + (u * 6.0).sin() * 3.0, -u * 26.0);
                    draw_circle(
                        q.x,
                        q.y,
                        4.0 + 6.0 * u,
                        gfx::with_alpha(brew, 0.45 * (1.0 - u) * p.pour),
                    );
                }
            }
        }

        // MWAHAHA! as a burst over the head.
        if p.cackle > 0.0 {
            let rise = 6.0 * p.cackle;
            let c = hc + vec2(0.0, -34.0 - rise);
            let w = measure_text("MWAHAHA!", None, 16, 1.0).width;
            draw_text(
                "MWAHAHA!",
                c.x - w * 0.5 + 1.0,
                c.y + 1.0,
                16.0,
                gfx::with_alpha(ink, p.cackle),
            );
            draw_text(
                "MWAHAHA!",
                c.x - w * 0.5,
                c.y,
                16.0,
                Color::new(1.0, 0.86, 0.30, p.cackle),
            );
        }
    }
}

/// Canvas size in local units, the feet's place in it, and the resolution
/// multiple the canvas is drawn at.
const CW: f32 = 220.0;
const CH: f32 = 152.0;
const FOOT_X: f32 = 110.0;
const FOOT_Y: f32 = 140.0;
const RES: f32 = 2.0;

/// The wrong sums the doctor writes under `2+2=5`.
const SUMS: [&str; 4] = ["1+1=3", "3x3=6", "5-2=7", "9/3=4"];

/// Where a hand ends up: `a` is how far the arm is raised, from -1 (swung back)
/// through 0 (hanging) to 1 (straight up), and `f` the way the doctor faces.
fn arm_hand(shoulder: Vec2, a: f32, f: f32, len: f32) -> Vec2 {
    let ang = a * PI * 0.9;
    shoulder + vec2(ang.sin() * len * f, ang.cos() * len)
}

/// How the doctor's body is posed: pure numbers, so the animation is easy to
/// check. Everything is bounded and a function of the action, its age and the
/// game clock.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    /// Body bounce in px (negative is up), and the sway of the shoulders and
    /// head in the facing direction.
    pub bob: f32,
    pub lean: f32,
    /// How far each arm is raised (-1 to 1): the flask arm, and the free one.
    pub arm_a: f32,
    pub arm_b: f32,
    /// Brow lift and waggle in px.
    pub brow: f32,
    /// Frizz growth, soot and flask fizz (0 to 1).
    pub frizz: f32,
    pub soot: f32,
    pub fizz: f32,
    /// Side to side shake of the head in px.
    pub shake: f32,
    /// Spark intensity on the coil (0 to 1).
    pub sparks: f32,
    /// How much of the wrong sum is written, and the pour, the telescope and the
    /// cackle word, each 0 to 1.
    pub scribble: f32,
    pub pour: f32,
    pub spy: f32,
    pub cackle: f32,
    /// Coat tail flap (0 to 1.5) and the stride phase, with whether they walk.
    pub flap: f32,
    pub stride: f32,
    pub walking: f32,
    /// Turns of the cart's wheels and the crank (radians).
    pub wheel: f32,
    pub crank: f32,
}

/// The pose for `action`, `t` seconds in, at game clock `clock`, having
/// travelled `travel` px.
pub fn pose(action: VillainAction, t: f32, clock: f32, travel: f32) -> Pose {
    use VillainAction::*;
    let ease = (t * 4.0).clamp(0.0, 1.0);
    let mut p = Pose {
        bob: (clock * 2.0).sin() * 0.8,
        arm_a: 0.38,
        arm_b: -0.1 + 0.05 * (clock * 1.7).sin(),
        flap: 0.25,
        stride: travel * 0.16,
        wheel: travel * 0.12,
        ..Pose::default()
    };
    match action {
        Pace => {
            p.walking = 1.0;
            p.bob = -(p.stride.sin().abs()) * 2.2;
            p.arm_b = -0.3 * p.stride.sin();
            p.flap = 0.7;
        }
        Scurry => {
            p.walking = 1.0;
            p.lean = 6.0;
            p.bob = -(p.stride.sin().abs()) * 3.4;
            p.arm_b = -0.7 * p.stride.sin();
            p.arm_a = 0.55;
            p.flap = 1.5;
            p.stride = travel * 0.3;
            p.brow = 2.0;
        }
        Tinker => {
            p.lean = 8.0 * ease;
            p.arm_b = 0.5 + 0.12 * (clock * 9.0).sin();
            p.sparks = 0.6 + 0.4 * (clock * 17.0).sin().abs();
            p.crank = clock * 6.0;
            p.brow = 0.8 * (clock * 5.0).sin();
        }
        Brew => {
            p.lean = 5.0 * ease;
            let pour = if t < 0.4 {
                t / 0.4
            } else {
                1.0 - ((t - 2.0) / 0.5).clamp(0.0, 1.0)
            };
            p.pour = pour.clamp(0.0, 1.0);
            p.arm_a = 0.38 + 0.5 * p.pour;
            p.brow = 1.5 * p.pour;
        }
        Cackle => {
            p.lean = -6.0 * ease;
            p.shake = (clock * 40.0).sin() * 1.1 * ease;
            p.brow = 2.6 * (clock * 14.0).sin();
            p.arm_a = 0.75;
            p.arm_b = 0.7;
            p.cackle =
                ((t * 6.0).min(1.0) * (1.0 - ((t - 2.2) / 0.5).clamp(0.0, 1.0))).clamp(0.0, 1.0);
        }
        Scribble => {
            p.lean = 8.0 * ease;
            p.arm_b = 0.62 + 0.08 * (clock * 12.0).sin();
            p.scribble = ((t - 0.4) / 1.6).clamp(0.0, 1.0);
        }
        Spy => {
            p.lean = 2.0 * ease;
            p.spy = ease;
            p.brow = 1.0;
        }
        Fluster => {
            p.frizz = ease;
            p.soot = (1.0 - t / 1.4).clamp(0.0, 1.0);
            p.fizz = ease;
            p.shake = (clock * 30.0).sin() * 1.4 * ease;
            p.brow = -3.0 * ease;
            p.arm_b = 0.4 + 0.2 * (clock * 11.0).sin();
        }
        Gloat => {
            p.lean = 3.0 * ease;
            p.arm_a = 0.55 + 0.06 * (clock * 12.0).sin();
            p.arm_b = 0.5 - 0.06 * (clock * 12.0).sin();
            p.brow = 2.0 * (clock * 8.0).sin();
        }
        Appear | Vanish => {}
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window 720 px high, which puts the player's anchor 475 px down.
    const VH: f32 = 720.0;

    fn p(x: f32, y: f32) -> Vec2 {
        vec2(x, y)
    }

    /// A doctor that's out on the road, for the tests that don't care about the
    /// gate.
    fn out_on_road() -> (Villain, Vec2, f32) {
        let mut v = Villain::new();
        let player = p(world::path_center(-400.0), -400.0);
        let cam = player.y - 475.0;
        v.update(0.0, cam, VH, player);
        assert!(v.is_here());
        // Let the puff clear and the first sighting be said, then start clean.
        for _ in 0..120 {
            v.update(1.0 / 60.0, cam, VH, player);
        }
        v.bubble = None;
        v.pending = None;
        v.since_bubble = 0.0;
        (v, player, cam)
    }

    #[test]
    fn the_shipped_bank_passes_its_own_check() {
        let bank = check_bank(include_str!("../../assets/villain_lines.json")).unwrap();
        for key in VillainCue::KEYS {
            assert!(bank.lines(key).len() >= MIN_LINES, "{key}");
        }
    }

    #[test]
    fn a_bad_bank_is_rejected() {
        let ok = include_str!("../../assets/villain_lines.json");
        // Not JSON, and not the shape.
        assert!(check_bank("nope").is_err());
        assert!(check_bank("[]").is_err());
        // An unknown cue.
        let unknown = ok.replacen("\"idle\"", "\"nap_time\"", 1);
        assert!(check_bank(&unknown).unwrap_err().contains("unknown cue"));
        // A missing cue.
        assert!(check_bank("{}").unwrap_err().contains("no lines"));
        // A line that is too long, one with a blocked word, a placeholder on the
        // wrong cue, and an action the game alone chooses.
        let long = "a".repeat(MAX_LINE_CHARS + 1);
        let with = |from: &str, to: &str| ok.replacen(from, to, 1);
        assert!(check_bank(&with(
            "Hee hee! My plan is working! Probably! I think!",
            &long
        ))
        .unwrap_err()
        .contains("over"));
        assert!(check_bank(&with(
            "Hee hee! My plan is working! Probably! I think!",
            "Mario says hi"
        ))
        .unwrap_err()
        .contains("blocked"));
        assert!(check_bank(&with(
            "Hee hee! My plan is working! Probably! I think!",
            "Level {level} hee"
        ))
        .unwrap_err()
        .contains("can't use"));
        assert!(
            check_bank(&with("\"action\": \"gloat\"", "\"action\": \"vanish\""))
                .unwrap_err()
                .contains("can't use")
        );
        // An unknown mood is a parse error, not a default.
        assert!(check_bank(&with("\"smug\"", "\"grumpy\"")).is_err());
        // Too few lines.
        let thin = r#"{"idle":[{"line":"x","mood":"smug","action":"pace"}]}"#;
        assert!(check_bank(thin).is_err());
    }

    #[test]
    fn a_line_with_a_number_still_fits_with_the_biggest_number() {
        // Checked at 99999, so a milestone of 100000 m can't overflow a bubble.
        let ok = include_str!("../../assets/villain_lines.json");
        // Exactly the cap as written ("{m} " is 4 characters), but 2 over once the
        // number is filled in.
        let long = format!("{{m}} {}", "x".repeat(MAX_LINE_CHARS - 4));
        assert_eq!(long.chars().count(), MAX_LINE_CHARS);
        let bad = ok.replacen("{m} metres already? My cart can barely keep up!", &long, 1);
        assert!(check_bank(&bad).is_err());
    }

    #[test]
    fn the_target_is_always_out_of_reach_and_near_the_road() {
        // Sweep the camera, the player (even one that's caught up with the
        // camera, as in a sprint) and the clock.
        for i in 0..60 {
            let player_y = -300.0 - i as f32 * 217.0;
            for lag in [0.0, 120.0, 300.0, 475.0] {
                let cam = player_y - lag;
                let pl = p(world::path_center(player_y), player_y);
                for w in 0..40 {
                    let t = target_pos(cam, VH, pl, w as f32 * 1.7);
                    assert!(t.y <= player_y - LEAD_PX + 0.001, "{t:?} vs {player_y}");
                    let (lo, hi) = world::walkable_x(t.y);
                    assert!(
                        t.x >= lo - VERGE_PX - 0.1 && t.x <= hi + VERGE_PX + 0.1,
                        "{t:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_sprinting_player_never_reaches_the_doctor() {
        let (mut v, mut player, mut cam) = out_on_road();
        let dt = 1.0 / 60.0;
        let mut closest = f32::MAX;
        for frame in 0..60 * 40 {
            // Sprint north at the top speed, with the game's camera easing.
            player.y -= 300.0 * dt;
            player.x = world::path_center(player.y);
            cam += ((player.y - 475.0) - cam) * (1.0 - (-dt * 6.0_f32).exp());
            v.update(dt, cam, VH, player);
            closest = closest.min(player.y - v.pos.y);
            assert!(
                v.pos.y < player.y - 60.0,
                "frame {frame}: doctor {} player {}",
                v.pos.y,
                player.y
            );
        }
        assert!(closest > 60.0, "closest {closest}");
    }

    #[test]
    fn a_doctor_the_player_gains_on_scurries() {
        let (mut v, player, cam) = out_on_road();
        // The player has closed right up behind them.
        v.pos.y = player.y - 100.0;
        v.update(0.1, cam, VH, player);
        assert_eq!(v.action(), VillainAction::Scurry);
        // Back to a comfortable lead, and they settle into something else.
        for _ in 0..600 {
            v.update(1.0 / 60.0, cam, VH, player);
        }
        assert_ne!(v.action(), VillainAction::Scurry);
    }

    #[test]
    fn they_arrive_at_the_gate_and_leave_for_the_hub() {
        let mut v = Villain::new();
        let hub = p(
            world::path_center(world::PLAYER_START_Y),
            world::PLAYER_START_Y,
        );
        v.cue(VillainCue::FlaskRight); // dropped: they aren't here
        for _ in 0..30 {
            v.update(1.0 / 60.0, hub.y - 475.0, VH, hub);
        }
        assert!(!v.visible() && !v.is_here());
        assert!(v.pending.is_none());

        let road = p(
            world::path_center(world::ROAD_START_Y - 5.0),
            world::ROAD_START_Y - 5.0,
        );
        v.update(1.0 / 60.0, road.y - 475.0, VH, road);
        assert!(v.is_here());
        assert_eq!(v.action(), VillainAction::Appear);
        // The first sighting is spoken once the puff clears, whatever the gap.
        for _ in 0..90 {
            v.update(1.0 / 60.0, road.y - 475.0, VH, road);
        }
        assert!(v.visible() && v.presence == 1.0);
        let said = v.saying().expect("a first sighting line");
        assert!(
            v.bank
                .lines("first_sighting")
                .iter()
                .any(|l| l.line == said),
            "{said}"
        );

        // Back south of the gate: gone, with their bubble, and no new sighting.
        for _ in 0..60 {
            v.update(1.0 / 60.0, hub.y - 475.0, VH, hub);
        }
        assert!(!v.visible() && v.saying().is_none());
        v.update(1.0 / 60.0, road.y - 475.0, VH, road);
        assert!(v.is_here());
        assert!(v.pending.is_none(), "the first sighting is once a run");
    }

    #[test]
    fn the_biggest_waiting_cue_is_the_one_said() {
        let (mut v, player, cam) = out_on_road();
        v.since_bubble = MIN_GAP_S;
        v.cue(VillainCue::FlaskRight);
        v.cue(VillainCue::LevelUp { level: 7 });
        v.cue(VillainCue::FlaskWrong); // lower: doesn't displace the level-up
        v.update(1.0 / 60.0, cam, VH, player);
        let said = v.saying().expect("a line");
        assert!(said.contains('7'), "{said}");
    }

    #[test]
    fn bubbles_keep_their_distance_and_stale_cues_lapse() {
        let (mut v, player, cam) = out_on_road();
        let dt = 1.0 / 60.0;
        let run = |v: &mut Villain, secs: f32| {
            for _ in 0..(secs / dt) as usize {
                v.update(dt, cam, VH, player);
            }
        };
        v.since_bubble = MIN_GAP_S;
        v.cue(VillainCue::BattleStart);
        v.update(dt, cam, VH, player);
        let first = v.saying().expect("the first line");

        // A small cue right after is held back for the gap, and has lapsed by the
        // time the gap is over (STALE_S is shorter than MIN_GAP_S), not said late.
        v.cue(VillainCue::FlaskWrong);
        run(&mut v, STALE_S - 1.0);
        assert!(v.pending.is_some(), "still waiting");
        assert_ne!(v.saying().as_deref(), Some(""), "{first}");
        run(&mut v, 2.0);
        assert!(v.pending.is_none(), "a stale small cue lapses");
        assert_eq!(
            v.said[VillainCue::FlaskWrong.index()],
            0,
            "and was never said"
        );

        // A big one waits however long it takes, and is said once the gap is up.
        v.cue(VillainCue::LevelUp { level: 2 });
        run(&mut v, MIN_GAP_S + 1.0);
        assert_eq!(v.said[VillainCue::LevelUp { level: 2 }.index()], 1);
        assert!(v.pending.is_none());
    }

    #[test]
    fn lines_rotate_and_fill_in_their_numbers() {
        let (mut v, ..) = out_on_road();
        let mut seen = Vec::new();
        for _ in 0..3 {
            v.cue_now(VillainCue::Streak { n: 10 });
            let s = v.saying().unwrap();
            assert!(s.contains("10") && !s.contains("{n}"), "{s}");
            seen.push(s);
        }
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 3, "three different lines in a row");
    }

    #[test]
    fn the_run_turns_into_cues() {
        let (mut v, ..) = out_on_road();
        let base = Snapshot {
            level: 1,
            streak: 0,
            meters: 0,
            health: 6,
        };
        v.observe(base);
        let cue_of = |v: &Villain| v.pending.map(|(c, _)| c);

        v.observe(Snapshot { streak: 4, ..base });
        assert_eq!(cue_of(&v), None, "4 is not a streak");
        v.observe(Snapshot { streak: 5, ..base });
        assert_eq!(cue_of(&v), Some(VillainCue::Streak { n: 5 }));
        v.pending = None;
        v.observe(Snapshot { streak: 5, ..base });
        assert_eq!(cue_of(&v), None, "once, not every frame");

        v.observe(Snapshot {
            streak: 6,
            meters: 101,
            ..base
        });
        assert_eq!(cue_of(&v), Some(VillainCue::Milestone { meters: 100 }));
        v.pending = None;
        v.observe(Snapshot {
            level: 2,
            streak: 6,
            meters: 101,
            health: 6,
        });
        assert_eq!(cue_of(&v), Some(VillainCue::LevelUp { level: 2 }));
        v.pending = None;
        v.observe(Snapshot {
            level: 2,
            streak: 6,
            meters: 101,
            health: 2,
        });
        assert_eq!(cue_of(&v), Some(VillainCue::LowHealth));
        v.pending = None;
        v.observe(Snapshot {
            level: 2,
            streak: 6,
            meters: 101,
            health: 1,
        });
        assert_eq!(cue_of(&v), None, "already low");
    }

    #[test]
    fn the_same_run_plays_the_same_every_time() {
        let run = || {
            let (mut v, mut player, mut cam) = out_on_road();
            let mut trace = Vec::new();
            for frame in 0..1500 {
                player.y -= 90.0 / 60.0;
                cam += ((player.y - 475.0) - cam) * 0.1;
                if frame % 400 == 0 {
                    v.cue(VillainCue::FlaskRight);
                }
                v.update(1.0 / 60.0, cam, VH, player);
                trace.push((v.pos.x.to_bits(), v.pos.y.to_bits(), v.action(), v.saying()));
            }
            trace
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn a_reset_goes_back_to_before_the_first_sighting() {
        let (mut v, ..) = out_on_road();
        v.cue_now(VillainCue::Idle);
        v.reset();
        assert!(!v.is_here() && !v.visible() && v.saying().is_none());
        assert!(!v.bank.lines("idle").is_empty(), "keeps its bank");
        assert!(!v.met);
    }

    #[test]
    fn every_pose_stays_in_bounds() {
        use VillainAction::*;
        for action in [
            Pace, Tinker, Brew, Cackle, Scribble, Spy, Fluster, Gloat, Scurry, Appear, Vanish,
        ] {
            for i in 0..400 {
                let (t, clock, travel) = (i as f32 * 0.02, i as f32 * 0.037, i as f32 * 3.0);
                let q = pose(action, t, clock, travel);
                for (name, v, lo, hi) in [
                    ("arm_a", q.arm_a, -1.0, 1.0),
                    ("arm_b", q.arm_b, -1.0, 1.0),
                    ("frizz", q.frizz, 0.0, 1.0),
                    ("soot", q.soot, 0.0, 1.0),
                    ("fizz", q.fizz, 0.0, 1.0),
                    ("sparks", q.sparks, 0.0, 1.0),
                    ("scribble", q.scribble, 0.0, 1.0),
                    ("pour", q.pour, 0.0, 1.0),
                    ("spy", q.spy, 0.0, 1.0),
                    ("cackle", q.cackle, 0.0, 1.0),
                    ("flap", q.flap, 0.0, 1.5),
                    ("lean", q.lean, -8.0, 10.0),
                    ("shake", q.shake, -2.0, 2.0),
                    ("brow", q.brow, -3.0, 3.0),
                    ("bob", q.bob, -4.0, 2.0),
                ] {
                    assert!((lo..=hi).contains(&v), "{action:?} {name} = {v} at t={t}");
                }
            }
        }
    }

    fn beat(line: &str, mood: Option<&str>, action: Option<&str>) -> RawBeat {
        RawBeat {
            line: line.into(),
            mood: mood.map(str::to_string),
            action: action.map(str::to_string),
        }
    }

    #[test]
    fn a_live_beat_is_mapped_defaulted_or_refused() {
        let l = beat_line(
            &beat("Curses!", Some("scheming"), Some("brew")),
            VillainCue::Idle,
        )
        .unwrap();
        assert_eq!((l.mood, l.action), (Mood::Scheming, VillainAction::Brew));
        // Unknown names fall back to the cue's mood and that mood's action.
        let l = beat_line(
            &beat("Hmm!", Some("sad"), Some("explode")),
            VillainCue::FlaskRight,
        )
        .unwrap();
        assert_eq!(
            (l.mood, l.action),
            (Mood::Flustered, VillainAction::Fluster)
        );
        // Only the game makes them appear or vanish.
        let l = beat_line(
            &beat("Poof!", Some("smug"), Some("vanish")),
            VillainCue::Idle,
        )
        .unwrap();
        assert_eq!(l.action, VillainAction::Gloat);
        // Blocked words, leftover placeholders, empty or over-long lines: the bank's.
        assert!(beat_line(&beat("I will kill the sums", None, None), VillainCue::Idle).is_none());
        assert!(beat_line(&beat("{m} metres!", None, None), VillainCue::Idle).is_none());
        assert!(beat_line(&beat("  ", None, None), VillainCue::Idle).is_none());
        assert!(beat_line(&beat(&"a".repeat(91), None, None), VillainCue::Idle).is_none());
    }

    #[test]
    fn a_live_line_is_said_when_it_arrives_and_the_bank_covers_a_late_one() {
        let (mut v, player, cam) = out_on_road();
        v.set_live(true);
        let step = |v: &mut Villain, secs: f32| {
            for _ in 0..(secs * 60.0) as usize {
                v.update(1.0 / 60.0, cam, VH, player);
            }
        };
        step(&mut v, 10.0);
        v.cue(VillainCue::BattleWon);
        assert_eq!(v.wants_line(), Some(VillainCue::BattleWon));
        v.mark_asked(VillainCue::BattleWon);
        assert_eq!(v.wants_line(), None);
        // Held while the line is out, then said once it's in.
        step(&mut v, 0.5);
        assert_eq!(v.saying(), None);
        let line = beat_line(
            &beat("My monster! My beautiful monster!", None, None),
            VillainCue::BattleWon,
        );
        v.supply(VillainCue::BattleWon, line);
        step(&mut v, 0.1);
        assert_eq!(
            v.saying().as_deref(),
            Some("My monster! My beautiful monster!")
        );
        assert_eq!((v.live_said, !v.recent_lines().is_empty()), (1, true));

        // A line that never comes: the bank speaks after LIVE_WAIT_S.
        step(&mut v, 10.0);
        v.cue(VillainCue::BattleWon);
        v.mark_asked(VillainCue::BattleWon);
        step(&mut v, LIVE_WAIT_S + 0.2);
        assert!(v.saying().is_some());
        assert_eq!(v.live_said, 1);
        // And a late line for a cue already said is dropped.
        v.supply(VillainCue::BattleWon, line_of("late"));
        assert_eq!(v.ready.as_ref().map(|l| l.line.as_str()), None);
    }

    fn line_of(s: &str) -> Option<Line> {
        beat_line(&beat(s, None, None), VillainCue::Idle)
    }

    #[test]
    fn live_switches_parse_from_the_env() {
        assert_eq!(
            LiveConfig::parse(None, None),
            LiveConfig {
                enabled: true,
                rpm: DEFAULT_RPM
            }
        );
        assert_eq!(
            LiveConfig::parse(Some("off"), Some("3")),
            LiveConfig {
                enabled: false,
                rpm: 3
            }
        );
        assert_eq!(LiveConfig::parse(Some("on"), Some("x")).rpm, DEFAULT_RPM);
    }

    #[test]
    fn dev_cue_names_cover_every_cue() {
        for key in VillainCue::KEYS {
            assert_eq!(
                VillainCue::from_dev(key).map(|c| c.key()),
                Some(key),
                "{key}"
            );
        }
        assert!(VillainCue::from_dev("nope").is_none());
    }
}
