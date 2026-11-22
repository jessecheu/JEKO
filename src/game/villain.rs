//! The mad scientist (FR23, milestone A): Dr. Nix Nullwell potters around at the
//! top of the road, always a little out of reach, and reacts to what the player
//! does with a speech bubble and an animation. Their lines come from
//! `assets/villain_lines.json`; nothing here calls the model.
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
use crate::llm::villain_gen::{self, BeatAsk, RunContext, MAX_SCHEME_CHARS};
pub use crate::llm::villain_gen::{Beat, Gadget, Mood, Scheme, VillainAction};
use macroquad::prelude::*;
use serde::Deserialize;
use std::cell::OnceCell;
use std::collections::BTreeMap;
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

/// The ones idle time chooses between. `Pace` is there twice because a doctor
/// who is always walking looks busiest.
const IDLE_ACTIONS: [VillainAction; 7] = [
    VillainAction::Pace,
    VillainAction::Tinker,
    VillainAction::Pace,
    VillainAction::Brew,
    VillainAction::Cackle,
    VillainAction::Scribble,
    VillainAction::Spy,
];

/// What idle time chooses between while the studio is building a monster
/// (FR23 C): the doctor is at work on it.
const WORKSHOP_ACTIONS: [VillainAction; 4] = [
    VillainAction::Tinker,
    VillainAction::Brew,
    VillainAction::Tinker,
    VillainAction::Pace,
];

impl Mood {
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
    Milestone {
        meters: u32,
    },
    FlaskRight,
    FlaskWrong,
    BattleStart,
    BattleWon,
    PlayerHurt,
    Streak {
        n: u32,
    },
    LevelUp {
        level: u32,
    },
    LowHealth,
    Idle,
    /// The doctor announces their `n`th scheme of the run (FR23 C).
    Scheme {
        n: u32,
    },
    /// The scheme running has come to nothing.
    SchemeFoiled,
    /// The studio has just finished a new monster (FR21), shown off by name.
    Creation,
}

impl VillainCue {
    /// Every cue's key in `assets/villain_lines.json`, in a fixed order.
    pub const KEYS: [&'static str; 14] = [
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
        "scheme",
        "scheme_foiled",
        "creation",
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
            VillainCue::Scheme { .. } => 11,
            VillainCue::SchemeFoiled => 12,
            VillainCue::Creation => 13,
        }
    }

    /// How much this deserves to be said over another waiting cue.
    fn priority(self) -> u8 {
        match self {
            VillainCue::FirstSighting => 9,
            VillainCue::LevelUp { .. } | VillainCue::Scheme { .. } => 8,
            VillainCue::Streak { .. } => 7,
            VillainCue::BattleWon | VillainCue::SchemeFoiled => 6,
            VillainCue::Milestone { .. } | VillainCue::LowHealth | VillainCue::Creation => 5,
            // A flask's reaction ranks with the hurt a wrong one costs, so the
            // reaction (cued after it) is what's said.
            VillainCue::PlayerHurt
            | VillainCue::BattleStart
            | VillainCue::FlaskRight
            | VillainCue::FlaskWrong => 4,
            VillainCue::Idle => 1,
        }
    }

    /// The mood a live line falls back on when the model's is unknown.
    fn mood(self) -> Mood {
        match self {
            VillainCue::FirstSighting | VillainCue::LowHealth => Mood::Smug,
            VillainCue::FlaskWrong | VillainCue::PlayerHurt | VillainCue::Creation => Mood::Gleeful,
            VillainCue::Milestone { .. } | VillainCue::FlaskRight | VillainCue::Streak { .. } => {
                Mood::Flustered
            }
            VillainCue::BattleStart | VillainCue::Idle | VillainCue::Scheme { .. } => {
                Mood::Scheming
            }
            VillainCue::BattleWon | VillainCue::LevelUp { .. } | VillainCue::SchemeFoiled => {
                Mood::FuriousButFunny
            }
        }
    }

    /// What happened, in a sentence, for the live prompt. `names` fills in the
    /// creature, the scheme and the new creation where the cue has one.
    fn happened(self, names: &Names) -> String {
        let monster = names.monster.as_deref().unwrap_or("one of your creatures");
        let scheme = names.scheme.as_deref().unwrap_or("your latest scheme");
        let creation = names.creation.as_deref().unwrap_or("a new creature");
        match self {
            VillainCue::FirstSighting => "The player has just come through the iron gate onto \
the road for the first time. Introduce yourself."
                .into(),
            VillainCue::Milestone { meters } => {
                format!("The player has walked {meters} metres up the road after you.")
            }
            VillainCue::FlaskRight => {
                "The player just solved one of your potion-flask maths problems.".into()
            }
            VillainCue::FlaskWrong => {
                "The player just got one of your potion-flask maths problems wrong.".into()
            }
            VillainCue::BattleStart => {
                format!("Your creature {monster} has just challenged the player to a maths battle.")
            }
            VillainCue::BattleWon => {
                format!("The player has just beaten your creature {monster} in a maths battle.")
            }
            VillainCue::PlayerHurt => "The player has just lost a heart.".into(),
            VillainCue::Streak { n } => {
                format!("The player has answered {n} maths problems in a row correctly.")
            }
            VillainCue::LevelUp { level } => format!("The player has just reached level {level}."),
            VillainCue::LowHealth => "The player is down to their last heart or so.".into(),
            VillainCue::Idle => "Nothing much has happened for a while. Mutter something about \
your inventions or your plan."
                .into(),
            VillainCue::Scheme { .. } => "Announce a brand-new scheme: a named plan with a \
gadget, themed on one maths topic. For a while your potion flasks will ask about that topic."
                .into(),
            VillainCue::SchemeFoiled => {
                format!("The player got past your scheme, {scheme}. It has fizzled out.")
            }
            VillainCue::Creation => {
                format!("Your lab has just finished a brand-new creature called {creation}. Show it off.")
            }
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
            "scheme" => VillainCue::Scheme { n: 1 },
            "scheme_foiled" | "foiled" => VillainCue::SchemeFoiled,
            "creation" => VillainCue::Creation,
            _ => return None,
        })
    }
}

/// The names a line may use: the last creature met, the scheme running (or
/// just over) and the newest creation.
#[derive(Clone, Debug, Default)]
struct Names {
    monster: Option<String>,
    scheme: Option<String>,
    creation: Option<String>,
}

/// The placeholder a cue's bank lines may use, and the widest thing it can
/// stand for (for the length check).
fn placeholder(key: &str) -> Option<(&'static str, usize)> {
    match key {
        "milestone" => Some(("{m}", 5)),
        "streak" => Some(("{n}", 5)),
        "level_up" => Some(("{level}", 5)),
        "scheme" | "scheme_foiled" => Some(("{scheme}", MAX_SCHEME_CHARS)),
        "creation" => Some(("{name}", crate::content::monster_spec::NAME_MAX)),
        _ => None,
    }
}

const PLACEHOLDERS: [&str; 5] = ["{m}", "{n}", "{level}", "{scheme}", "{name}"];

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
/// bubble's budget; live beats use the same cap, `villain_gen::MAX_LINE_CHARS`).
pub const MIN_LINES: usize = 2;
pub const MAX_LINES: usize = 8;
pub const MAX_LINE_CHARS: usize = villain_gen::MAX_LINE_CHARS;

/// The compiled-in bank. Panics at startup on a bad one, like the dialogue bank,
/// and a test runs `check_bank` on the shipped file so that can't happen.
pub fn load_bank() -> Bank {
    check_bank(include_str!("../../assets/villain_lines.json"))
        .expect("assets/villain_lines.json must pass villain::check_bank")
}

/// Parses and checks a bank of lines: an object with one array per cue key
/// (every key present, none unknown), each with 2 to 8 lines of at most 90
/// characters (counted with the widest thing a placeholder could hold), no
/// word on the blocklist (`content::vet`), a placeholder (`{m}`, `{n}`,
/// `{level}`, `{scheme}`, `{name}`) only on the cue that has it, and no
/// `appear` or `vanish` action, which only the game chooses. The
/// first problem found is the error. `validate_content` runs it on the
/// shipped file.
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
        let allowed = placeholder(key);
        for l in lines {
            if l.line.trim().is_empty() {
                return Err(format!("{key} has an empty line"));
            }
            for p in PLACEHOLDERS {
                if l.line.contains(p) && allowed.map(|(a, _)| a) != Some(p) {
                    return Err(format!("{key}: {:?} can't use {p}", l.line));
                }
            }
            let widest = match allowed {
                Some((p, w)) => l.line.replace(p, &"W".repeat(w)),
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
            if !l.action.speakable() {
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

/// Schemes come every this many metres (FR23 C).
pub const SCHEME_EVERY_M: u32 = 300;
/// How long a cue that's due waits for its live line before a bank line is
/// said instead (game seconds). Cerebras usually answers in well under one.
pub const LIVE_WAIT_S: f32 = 2.0;

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

/// A cue waiting to be said.
#[derive(Clone, Debug)]
struct Pending {
    cue: VillainCue,
    /// Seconds it has waited, and seconds it has been due but held for its
    /// live line.
    age: f32,
    held: f32,
    /// Its number, so a live line that comes back late for a cue that has
    /// since been replaced is dropped, not said over the new one.
    id: u32,
    /// The live line, once it's here.
    beat: Option<Beat>,
    /// A live line has been asked for, and whether that ask has failed.
    asked: bool,
    failed: bool,
}

/// Where the doctor's line came from, counted for the debug readout and the
/// checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Said {
    pub live: u32,
    pub bank: u32,
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
    pending: Option<Pending>,
    /// The next cue's number. Kept across a reset, so a late reply from the
    /// last run can't match a cue in this one.
    next_id: u32,
    /// Live lines are expected (FR23 B): a key, `JEKO_VILLAIN` on and the
    /// chatter setting on. A due cue then waits a moment for its live line.
    live: bool,
    since_bubble: f32,
    /// Times each cue has been said from the bank, so its lines rotate.
    said: [u32; VillainCue::KEYS.len()],
    /// How many lines came live and how many from the bank.
    sources: Said,
    /// Picks made, to vary what they do next without a random generator.
    picks: u32,
    /// Their own slow clock for wandering; runs only while they're on the move.
    wander: f32,
    /// Distance covered, which turns the cart's wheels and drives the stride.
    travel: f32,
    /// The wrong sum on the chalkboard, and which one.
    sum: usize,
    prev: Snapshot,
    /// The scheme running now (FR23 C), drawn on the cart; the one just
    /// announced, for `Game` to start; and the names lines may use.
    scheme: Option<Scheme>,
    announced: Option<Scheme>,
    names: Names,
    /// The studio is building a monster: idle time goes to the workbench.
    workshop: bool,
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
            next_id: 1,
            live: false,
            since_bubble: 0.0,
            said: [0; VillainCue::KEYS.len()],
            sources: Said::default(),
            picks: 0,
            wander: 0.0,
            travel: 0.0,
            sum: 0,
            prev: Snapshot::default(),
            scheme: None,
            announced: None,
            names: Names::default(),
            workshop: false,
            canvas: OnceCell::new(),
        }
    }

    /// Back to before the first sighting, for a new run. Keeps the bank, the
    /// canvas, the cue numbering and whether live lines are on.
    pub fn reset(&mut self) {
        let bank = std::mem::replace(&mut self.bank, Bank(BTreeMap::new()));
        let canvas = std::mem::take(&mut self.canvas);
        *self = Self {
            canvas,
            next_id: self.next_id,
            live: self.live,
            ..Self::with_bank(bank)
        };
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

    /// How many lines came live and how many from the bank, this run.
    pub fn sources(&self) -> Said {
        self.sources
    }

    /// The scheme running now, if any (FR23 C).
    pub fn scheme(&self) -> Option<&Scheme> {
        self.scheme.as_ref()
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

    // -----------------------------------------------------------------------
    // Cues and lines

    /// Something happened. The doctor remarks on it when they're next free to,
    /// unless it's already stale or something bigger came along. Dropped while
    /// they're away (the hub stays calm). Returns the cue's number when it's
    /// the one now waiting, for a live line to be matched to it.
    pub fn cue(&mut self, cue: VillainCue) -> Option<u32> {
        if !self.here {
            return None;
        }
        let better = match &self.pending {
            Some(p) => cue.priority() >= p.cue.priority(),
            None => true,
        };
        if !better {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.pending = Some(Pending {
            cue,
            age: 0.0,
            held: 0.0,
            id,
            beat: None,
            asked: false,
            failed: false,
        });
        Some(id)
    }

    /// `cue`, with its live line already in hand (a flask's prefetched
    /// reaction, FR23 B).
    pub fn cue_with(&mut self, cue: VillainCue, beat: Beat) -> Option<u32> {
        let id = self.cue(cue)?;
        self.offer(id, beat);
        Some(id)
    }

    /// Says `cue` now, whatever else is going on, for the dev hook.
    pub fn cue_now(&mut self, cue: VillainCue) {
        if self.here {
            self.speak(cue, None);
        }
    }

    /// Announces `scheme` now, for the dev hook (`JEKO_DEV=scheme:<topic>`).
    pub fn announce_now(&mut self, scheme: Scheme) {
        if self.here {
            let line = format!("Behold: {}!", scheme.name);
            let beat = Beat {
                line,
                mood: Mood::Scheming,
                action: VillainAction::Cackle,
                scheme: Some(scheme),
            };
            self.speak(VillainCue::Scheme { n: 0 }, Some(beat));
        }
    }

    /// Whether live lines are expected (FR23 B). `Game` sets it every frame.
    pub fn set_live(&mut self, live: bool) {
        self.live = live;
    }

    /// The waiting cue that has no live line and hasn't asked for one, with
    /// its number: what the game's voice should ask the model for next.
    pub fn wants_line(&self) -> Option<(u32, VillainCue)> {
        let p = self.pending.as_ref()?;
        (self.here && p.beat.is_none() && !p.asked && !p.failed).then_some((p.id, p.cue))
    }

    /// A live line has been asked for cue `id`.
    pub fn mark_asked(&mut self, id: u32) {
        if let Some(p) = self.pending.as_mut().filter(|p| p.id == id) {
            p.asked = true;
        }
    }

    /// The live line for cue `id` has come back. Dropped if that cue is no
    /// longer the one waiting.
    pub fn offer(&mut self, id: u32, beat: Beat) -> bool {
        match self.pending.as_mut().filter(|p| p.id == id) {
            Some(p) => {
                p.beat = Some(beat);
                true
            }
            None => false,
        }
    }

    /// No live line is coming for cue `id` (the call failed or was turned
    /// down, or the rate cap is reached): say a bank line when it's due.
    pub fn give_up(&mut self, id: u32) {
        if let Some(p) = self.pending.as_mut().filter(|p| p.id == id) {
            p.failed = true;
        }
    }

    /// What a live prompt needs for `cue`, with `ctx` from the game and the
    /// topics a scheme may pick (`topics`, for a `Scheme` cue only).
    pub fn beat_ask(&self, cue: VillainCue, mut ctx: RunContext, topics: &[&str]) -> BeatAsk {
        if ctx.scheme.is_none() {
            ctx.scheme = self.scheme.as_ref().map(|s| s.name.clone());
        }
        let names = Names {
            monster: ctx
                .last_monster
                .clone()
                .or_else(|| self.names.monster.clone()),
            ..self.names.clone()
        };
        BeatAsk {
            cue: cue.key(),
            happened: cue.happened(&names),
            mood: cue.mood(),
            ctx,
            scheme_topics: match cue {
                VillainCue::Scheme { .. } => topics.iter().map(|t| t.to_string()).collect(),
                _ => Vec::new(),
            },
        }
    }

    /// The live prompt's ask for a flask's reaction, right or wrong.
    pub fn reaction_ask(&self, correct: bool, ctx: RunContext) -> BeatAsk {
        let cue = if correct {
            VillainCue::FlaskRight
        } else {
            VillainCue::FlaskWrong
        };
        self.beat_ask(cue, ctx, &[])
    }

    /// The creature the player last met, for lines that name it.
    pub fn note_monster(&mut self, name: &str) {
        self.names.monster = Some(name.to_string());
    }

    /// The studio is (or isn't) building a monster right now (FR23 C).
    pub fn set_workshop(&mut self, busy: bool) {
        self.workshop = busy;
    }

    /// The studio has finished a new monster: the doctor holds up the flask
    /// and shows it off by name.
    pub fn creation(&mut self, name: &str) {
        if self.here {
            self.note_creation(name);
            self.cue(VillainCue::Creation);
        }
    }

    /// The newest creation's name, for the lines that show it off.
    pub fn note_creation(&mut self, name: &str) {
        self.names.creation = Some(name.to_string());
    }

    /// The scheme just announced, once, for `Game` to start (FR23 C).
    pub fn take_announced(&mut self) -> Option<Scheme> {
        self.announced.take()
    }

    /// The scheme has run its course: its gadget goes and the doctor says so.
    pub fn end_scheme(&mut self) {
        if let Some(s) = self.scheme.take() {
            self.names.scheme = Some(s.name);
            self.cue(VillainCue::SchemeFoiled);
        }
    }

    /// Turns changes in the run into cues: a level, a streak of 5 or each 10, each
    /// 100 m, a scheme every 300 m, and hearts running low. Called every unpaused
    /// frame. `Game` fires the ones that belong to a single event (an answer, a
    /// hit, a battle).
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
        // A scheme every 300 m, when none is running (it outranks the milestone
        // at the same mark).
        if now.meters / SCHEME_EVERY_M > prev.meters / SCHEME_EVERY_M && self.scheme.is_none() {
            self.cue(VillainCue::Scheme {
                n: now.meters / SCHEME_EVERY_M,
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
                self.cue(VillainCue::FirstSighting);
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
        if let Some(p) = &mut self.pending {
            p.age += dt;
            if p.age > STALE_S && p.cue.priority() < 6 {
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
        } else if let Some(p) = &mut self.pending {
            let free = self.since_bubble >= MIN_GAP_S || p.cue.priority() >= 9;
            if free && !appearing {
                // Due. With live lines on, wait a moment for this one's.
                let hold = self.live && p.beat.is_none() && !p.failed && p.held < LIVE_WAIT_S;
                if hold {
                    p.held += dt;
                } else if let Some(p) = self.pending.take() {
                    self.speak(p.cue, p.beat);
                }
            }
        } else if self.since_bubble >= IDLE_S && !appearing {
            // A mutter goes through the queue too, so it can be live.
            self.cue(VillainCue::Idle);
            self.since_bubble = IDLE_S - MIN_GAP_S - STALE_S;
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
        let list: &[VillainAction] = if self.workshop {
            &WORKSHOP_ACTIONS
        } else {
            &IDLE_ACTIONS
        };
        let n = list.len();
        let r = gfx::hash01(self.picks as u64 * 7919 + 17);
        let mut i = (r * n as f32) as usize % n;
        if list[i] == self.action {
            i = (i + 1) % n;
        }
        let len = 3.5 + 3.0 * gfx::hash01(self.picks as u64 * 104_729 + 3);
        self.set_action(list[i], len);
    }

    /// A bank line for `cue`, its placeholder filled in, rotating through the
    /// cue's lines. `scheme` names a scheme being announced.
    fn bank_beat(&mut self, cue: VillainCue, scheme: Option<&Scheme>) -> Option<Beat> {
        let lines = self.bank.lines(cue.key());
        if lines.is_empty() {
            return None;
        }
        let k = cue.index();
        let start = (gfx::hash01(k as u64 + 5) * lines.len() as f32) as usize;
        let pick = lines[(start + self.said[k] as usize) % lines.len()].clone();
        self.said[k] += 1;
        let fill = match cue {
            VillainCue::Milestone { meters } => meters.to_string(),
            VillainCue::Streak { n } => n.to_string(),
            VillainCue::LevelUp { level } => level.to_string(),
            VillainCue::Scheme { .. } => scheme.map(|s| s.name.clone()).unwrap_or_default(),
            VillainCue::SchemeFoiled => self.names.scheme.clone().unwrap_or_default(),
            VillainCue::Creation => self.names.creation.clone().unwrap_or_default(),
            _ => String::new(),
        };
        let line = match placeholder(cue.key()) {
            Some((p, _)) => pick.line.replace(p, &fill),
            None => pick.line,
        };
        Some(Beat {
            line,
            mood: pick.mood,
            action: pick.action,
            scheme: None,
        })
    }

    /// The scheme a scheme cue starts with no live one: the fallback for a
    /// topic that fits the player's level, rotating with the scheme's number.
    fn fallback_scheme(&self, n: u32) -> Scheme {
        let topics = crate::content::prompts::topics_for_level(self.prev.level.max(1));
        villain_gen::fallback_scheme(topics[n as usize % topics.len()])
    }

    /// Opens a bubble for `cue` and acts out its line: the live one when it
    /// came, else one from the bank. A scheme cue starts its scheme.
    fn speak(&mut self, cue: VillainCue, live: Option<Beat>) {
        let mut scheme = None;
        if let VillainCue::Scheme { n } = cue {
            scheme = Some(
                live.as_ref()
                    .and_then(|b| b.scheme.clone())
                    .unwrap_or_else(|| self.fallback_scheme(n)),
            );
        }
        // A live scheme line without its scheme can't be said.
        let live = live.filter(|b| scheme.is_none() || b.scheme.is_some());
        let beat = match live {
            Some(b) => {
                self.sources.live += 1;
                b
            }
            None => match self.bank_beat(cue, scheme.as_ref()) {
                Some(b) => {
                    self.sources.bank += 1;
                    b
                }
                None => return,
            },
        };
        if let Some(s) = scheme {
            self.scheme = Some(s.clone());
            self.announced = Some(s);
        }
        self.mood = beat.mood;
        self.bubble = Some(Bubble {
            chars: beat.line.chars().count(),
            lines: gfx::wrap_text(&beat.line, 34),
            t: 0.0,
        });
        self.since_bubble = 0.0;
        self.set_action(beat.action, REACT_S);
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
        self.draw_gadget(ground, time, true);
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

        self.draw_gadget(ground, time, false);

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

    /// The running scheme's gadget (FR23 C), in screen space around the cart:
    /// the fog behind the doctor (`behind`), the bubbles and the banner in
    /// front. Cosmetic, and nothing in it flashes: it drifts on the game clock
    /// and fades with the doctor.
    fn draw_gadget(&self, ground: Vec2, time: f32, behind: bool) {
        let Some(s) = &self.scheme else {
            return;
        };
        let a = self.presence;
        let colour = gadget_colour(s.gadget);
        let cart = ground + vec2(64.0 * self.face, -30.0);
        match (s.gadget, behind) {
            (Gadget::FogMachine, true) => {
                // Puffs that roll off the cart both ways, growing as they fade.
                for i in 0..10 {
                    let u = (time * 0.16 + i as f32 / 10.0).fract();
                    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                    let c = cart
                        + vec2(
                            side * (8.0 + u * 150.0) + (time * 0.6 + i as f32).sin() * 6.0,
                            22.0 + u * 8.0,
                        );
                    let r = 16.0 + u * 38.0;
                    let fade = (u * 5.0).min(1.0) * (1.0 - u);
                    gfx::ellipse(c, r, r * 0.42, gfx::with_alpha(colour, 0.32 * fade * a));
                }
            }
            (Gadget::BubbleRain, false) => {
                for i in 0..12u64 {
                    let u = (time * 0.3 + gfx::hash01(i)).fract();
                    let x = cart.x
                        + (gfx::hash01(i + 20) - 0.5) * 80.0
                        + (time * 1.4 + i as f32).sin() * 5.0;
                    let y = cart.y - 24.0 - u * 110.0;
                    let r = 2.5 + 3.5 * gfx::hash01(i + 40);
                    let al = a * (1.0 - u) * 0.85;
                    draw_circle_lines(x, y, r, 1.2, gfx::with_alpha(colour, al));
                    draw_circle(
                        x - r * 0.35,
                        y - r * 0.35,
                        r * 0.28,
                        gfx::with_alpha(WHITE, al * 0.8),
                    );
                }
            }
            (Gadget::Banner, false) => {
                // A pole on the cart's far end with a pennant naming the scheme.
                let foot = cart + vec2(-24.0 * self.face, 0.0);
                let top = foot - vec2(0.0, 52.0);
                gfx::line(foot, top, 2.6, gfx::with_alpha(INK, a));
                draw_circle(top.x, top.y, 2.6, gfx::with_alpha(gfx::LANTERN, a));
                let size = 14.0;
                let w = measure_text(&s.name, None, size as u16, 1.0).width + 18.0;
                let h = 20.0;
                let dir = -self.face;
                let wave = |x: f32| (time * 2.2 - x * 0.05).sin() * 2.0;
                let p0 = top + vec2(0.0, 4.0);
                let far = p0.x + dir * w;
                gfx::quad(
                    p0,
                    vec2(far, p0.y + wave(w)),
                    vec2(far + dir * -6.0, p0.y + h * 0.5 + wave(w)),
                    p0 + vec2(0.0, h),
                    gfx::with_alpha(gfx::shade(colour, 0.55), a),
                );
                gfx::quad(
                    p0 + vec2(0.0, h * 0.5),
                    vec2(far + dir * -6.0, p0.y + h * 0.5 + wave(w)),
                    vec2(far, p0.y + h + wave(w)),
                    p0 + vec2(0.0, h),
                    gfx::with_alpha(gfx::shade(colour, 0.45), a),
                );
                let tx = if dir > 0.0 { p0.x + 8.0 } else { far + 10.0 };
                draw_text(
                    &s.name,
                    tx,
                    p0.y + 14.5 + wave(w * 0.5),
                    size,
                    Color::new(1.0, 0.96, 0.86, a),
                );
            }
            _ => {}
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
        // A glowing-flask scheme tints the brew its colour (FR23 C).
        let glow_flask = self
            .scheme
            .as_ref()
            .is_some_and(|s| s.gadget == Gadget::GlowFlask);
        let brew = if glow_flask {
            gadget_colour(Gadget::GlowFlask)
        } else {
            self.mood.brew()
        };

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
            // over on a fluster. A glowing-flask scheme lights it up, steadily
            // under reduce flashing.
            if glow_flask {
                let pulse = if rf {
                    0.8
                } else {
                    0.65 + 0.35 * (time * 2.0).sin()
                };
                gfx::glow(hand, 26.0, 22.0, gfx::with_alpha(brew, 0.55 * pulse));
            }
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
        Unveil => {
            // The flask held up high, fizzing, to show off a new creation.
            p.lean = -4.0 * ease;
            p.arm_a = 0.38 + 0.55 * ease;
            p.fizz = ease;
            p.brow = 2.2 * ease;
        }
        Appear | Vanish => {}
    }
    p
}

/// The colour of a scheme's gadget.
fn gadget_colour(g: Gadget) -> Color {
    match g {
        Gadget::FogMachine => gfx::LAB_GREEN,
        Gadget::BubbleRain => Color::new(0.62, 0.86, 1.0, 1.0),
        Gadget::Banner => Color::new(0.68, 0.45, 1.0, 1.0),
        Gadget::GlowFlask => Color::new(1.0, 0.52, 0.88, 1.0),
    }
}

#[cfg(test)]
impl Villain {
    /// A doctor out on the road with nothing waiting, for other modules' tests.
    pub(crate) fn out_for_test() -> Villain {
        let mut v = Villain::new();
        v.here = true;
        v.met = true;
        v.presence = 1.0;
        v.action = VillainAction::Pace;
        v
    }

    /// The live line the waiting cue has, if any.
    pub(crate) fn pending_line(&self) -> Option<String> {
        self.pending.as_ref()?.beat.as_ref().map(|b| b.line.clone())
    }

    /// The waiting cue has given up on its live line.
    pub(crate) fn pending_failed(&self) -> bool {
        self.pending.as_ref().is_some_and(|p| p.failed)
    }
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
        v.sources = Said::default();
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
    fn a_wrong_flask_is_reacted_to_rather_than_the_heart_it_cost() {
        let (mut v, ..) = out_on_road();
        // `submit_answer` hurts first, then cues the reaction.
        v.cue(VillainCue::PlayerHurt);
        assert!(v
            .cue_with(VillainCue::FlaskWrong, live_beat("Hee hee!"))
            .is_some());
        assert_eq!(
            v.pending.as_ref().map(|p| p.cue),
            Some(VillainCue::FlaskWrong)
        );
        assert_eq!(v.pending_line().as_deref(), Some("Hee hee!"));
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
        let cue_of = |v: &Villain| v.pending.as_ref().map(|p| p.cue);

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
        for action in VillainAction::ALL {
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

    fn live_beat(line: &str) -> Beat {
        Beat {
            line: line.into(),
            mood: Mood::Gleeful,
            action: VillainAction::Cackle,
            scheme: None,
        }
    }

    #[test]
    fn with_live_lines_on_a_due_cue_waits_a_moment_for_its_line() {
        let (mut v, player, cam) = out_on_road();
        let dt = 1.0 / 60.0;
        v.set_live(true);
        v.since_bubble = MIN_GAP_S;
        let id = v.cue(VillainCue::BattleWon).unwrap();
        assert_eq!(v.wants_line(), Some((id, VillainCue::BattleWon)));
        v.mark_asked(id);
        assert_eq!(v.wants_line(), None, "asked once");
        for _ in 0..30 {
            v.update(dt, cam, VH, player);
        }
        assert!(v.saying().is_none(), "held for the live line");
        assert!(v.offer(id, live_beat("Live and furious!")));
        v.update(dt, cam, VH, player);
        assert_eq!(v.saying().as_deref(), Some("Live and furious!"));
        assert_eq!(v.action(), VillainAction::Cackle);
        assert_eq!(v.sources(), Said { live: 1, bank: 0 });
    }

    #[test]
    fn a_live_line_that_never_comes_gives_way_to_the_bank() {
        let (mut v, player, cam) = out_on_road();
        let dt = 1.0 / 60.0;
        v.set_live(true);
        v.since_bubble = MIN_GAP_S;
        let id = v.cue(VillainCue::BattleWon).unwrap();
        v.mark_asked(id);
        for _ in 0..((LIVE_WAIT_S + 0.2) / dt) as usize {
            v.update(dt, cam, VH, player);
        }
        let said = v.saying().expect("a bank line after the wait");
        assert!(v.bank.lines("battle_won").iter().any(|l| l.line == said));
        // A failed ask doesn't wait at all.
        v.bubble = None;
        v.since_bubble = MIN_GAP_S;
        let id = v.cue(VillainCue::BattleWon).unwrap();
        v.give_up(id);
        v.update(dt, cam, VH, player);
        assert!(v.saying().is_some());
        // And with live lines off nothing waits either.
        v.set_live(false);
        v.bubble = None;
        v.since_bubble = MIN_GAP_S;
        v.cue(VillainCue::BattleWon);
        v.update(dt, cam, VH, player);
        assert!(v.saying().is_some());
        assert_eq!(v.sources().live, 0);
        // A line for a cue that's no longer waiting is refused.
        assert!(!v.offer(id, live_beat("too late")));
    }

    #[test]
    fn a_scheme_every_300_m_starts_with_a_topic_that_fits_the_level() {
        let (mut v, ..) = out_on_road();
        let base = Snapshot {
            level: 3,
            streak: 0,
            meters: 290,
            health: 6,
        };
        v.observe(base);
        v.observe(Snapshot {
            meters: 301,
            ..base
        });
        let cue = v.pending.as_ref().map(|p| p.cue);
        assert_eq!(
            cue,
            Some(VillainCue::Scheme { n: 1 }),
            "outranks the milestone"
        );
        v.pending = None;
        v.cue_now(VillainCue::Scheme { n: 1 });
        let s = v.take_announced().expect("announced");
        assert!(crate::content::prompts::topics_for_level(3).contains(&s.topic.as_str()));
        assert_eq!(v.scheme(), Some(&s));
        assert!(
            v.saying().unwrap().contains(&s.name),
            "the bank line names it"
        );
        assert!(v.take_announced().is_none(), "once");
        // No new scheme while one runs.
        v.observe(Snapshot {
            meters: 601,
            ..base
        });
        assert_ne!(
            v.pending.as_ref().map(|p| p.cue),
            Some(VillainCue::Scheme { n: 2 })
        );
        // Over: it goes, and the doctor says so by name.
        v.pending = None;
        v.end_scheme();
        assert!(v.scheme().is_none());
        v.cue_now(VillainCue::SchemeFoiled);
        assert!(v.saying().unwrap().contains(&s.name));
    }

    #[test]
    fn a_live_scheme_is_used_and_a_live_line_without_one_is_not() {
        let (mut v, ..) = out_on_road();
        let scheme = Scheme {
            name: "Operation Soggy Sums".into(),
            topic: "division".into(),
            gadget: Gadget::BubbleRain,
        };
        v.speak(
            VillainCue::Scheme { n: 2 },
            Some(Beat {
                scheme: Some(scheme.clone()),
                ..live_beat("Behold, Operation Soggy Sums!")
            }),
        );
        assert_eq!(v.take_announced(), Some(scheme));
        assert_eq!(v.saying().as_deref(), Some("Behold, Operation Soggy Sums!"));
        // A scheme line that lost its scheme: the bank and a fallback scheme.
        v.scheme = None;
        v.speak(VillainCue::Scheme { n: 2 }, Some(live_beat("Behold!")));
        let s = v.take_announced().unwrap();
        assert!(v.saying().unwrap().contains(&s.name));
    }

    #[test]
    fn a_new_creation_is_shown_off_by_name_and_the_workshop_keeps_them_busy() {
        let (mut v, player, cam) = out_on_road();
        v.since_bubble = MIN_GAP_S;
        v.creation("Fizzwick");
        v.update(1.0 / 60.0, cam, VH, player);
        assert!(v.saying().unwrap().contains("Fizzwick"));
        assert_eq!(v.action(), VillainAction::Unveil);

        v.set_workshop(true);
        for _ in 0..40 {
            v.next_action();
            assert!(WORKSHOP_ACTIONS.contains(&v.action()), "{:?}", v.action());
        }
    }

    #[test]
    fn the_new_cues_placeholders_are_checked() {
        let ok = include_str!("../../assets/villain_lines.json");
        let line = "Behold my newest plan: {scheme}! Hee hee!";
        // A scheme's name is counted at its widest, so a long line can't overflow.
        let long = format!("{{scheme}} {}", "x".repeat(MAX_LINE_CHARS - 20));
        assert!(check_bank(&ok.replacen(line, &long, 1)).is_err());
        // {name} on a scheme line, {scheme} on an idle one: no.
        assert!(check_bank(&ok.replacen(line, "Hello {name}", 1)).is_err());
        let idle = "La la la, nothing suspicious here. Just me and my wrench.";
        assert!(check_bank(&ok.replacen(idle, "{scheme} la la", 1)).is_err());
    }

    #[test]
    fn every_topic_a_scheme_can_pick_has_offline_questions_and_a_fallback() {
        // A scheme steers flasks onto its topic (FR23 C); offline, the bank has to
        // have that topic or the scheme would steer nothing.
        let bank = crate::llm::fallback_bank();
        for level in 1..=5 {
            for topic in crate::content::prompts::topics_for_level(level) {
                assert!(
                    bank.iter()
                        .any(|t| crate::llm::buffer::same_topic(&t.topic, topic)),
                    "no offline question on {topic}"
                );
                let s = villain_gen::fallback_scheme(topic);
                assert!(s.name.chars().count() <= MAX_SCHEME_CHARS);
                assert!(crate::content::vet::vet_text(&s.name).is_ok());
            }
        }
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
