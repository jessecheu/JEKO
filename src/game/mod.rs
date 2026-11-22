pub mod battle;
pub mod bow;
pub mod curse;
pub mod decor;
pub mod dialogue;
pub mod field_guide;
pub mod flicker;
pub mod gfx;
pub mod goalpost;
pub mod health;
pub mod manor;
pub mod map;
pub mod monster;
pub mod monster_rig;
pub mod player;
pub mod prefetch;
pub mod shop;
pub mod skills;
pub mod task_trigger;
pub mod villain;
pub mod world;

use crate::content::studio::{self, MonsterJob, Studio, StudioEvent, StudioStatus, TipsJob};
use crate::content::{ContentEvent, Roster};
use crate::llm::activity::{BarView, GenActivity, JobState, Stock};
use crate::llm::buffer::{Source, WaitStats};
use crate::llm::cerebras_client::CerebrasClient;
use crate::llm::usage::RequestKind;
use crate::llm::MathTask;
use crate::progression::Progression;
use battle::{Battle, Phase};
use curse::Curse;
use flicker::FlickerKind;
use goalpost::Tracker;
use health::Health;
use macroquad::prelude::*;
use monster::{MonsterKind, Species};
use player::Player;
use prefetch::{Ask, Prefetch, PrefetchConfig};
use std::sync::mpsc::{Receiver, TryRecvError};
use task_trigger::{Arrival, TriggerState};
use villain::{Villain, VillainCue};
use world::{SpotState, View, World};

/// The darkness vignette around the player (FR24): clear within the first
/// radius, fading to `NIGHT_STRENGTH` of the night colour by the second.
const NIGHT_CLEAR_R: f32 = 190.0;
const NIGHT_DARK_R: f32 = 620.0;
const NIGHT_STRENGTH: f32 = 0.5;

/// Where the player sits on screen, as a fraction of its height from the
/// top. Low, so most of the screen shows the road ahead of them.
const CAMERA_ANCHOR: f32 = 0.66;
/// How quickly the camera closes the gap to its target (per second); high
/// enough to keep up with the player, low enough to glide rather than snap.
const CAMERA_FOLLOW: f32 = 6.0;
/// The one "do the thing" key: opens whatever's in reach, and jumps to the
/// answer field once a task is open, so play never needs the mouse.
pub const INTERACT_KEY: KeyCode = KeyCode::E;
/// Toggles the pause (FR6).
pub const PAUSE_KEY: KeyCode = KeyCode::P;
/// Opens and closes the discovery map (FR27); Esc also closes it.
pub const MAP_KEY: KeyCode = KeyCode::M;
/// Opens and closes the settings modal (FR7); Esc also closes it.
pub const SETTINGS_KEY: KeyCode = KeyCode::O;
/// Opens and closes the field guide (FR22); Esc also closes it.
pub const GUIDE_KEY: KeyCode = KeyCode::G;

/// Player-facing switches, edited in the settings modal (FR7) and read by
/// the systems they control. Not saved between runs yet.
#[derive(Clone, Debug)]
pub struct Settings {
    pub show_token_panel: bool,
    pub show_hud: bool,
    /// The small map in the top right corner (FR27).
    pub show_minimap: bool,
    /// Swaps flicker effects (spawn, damage) for a steady tint.
    pub reduce_flashing: bool,
    /// The creation bar at the top centre (FR33).
    pub show_creation_bar: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_token_panel: true,
            show_hud: true,
            show_minimap: true,
            reduce_flashing: false,
            show_creation_bar: true,
        }
    }
}

/// How a question's arrival shows on the creation bar.
fn job_state(arrival: Arrival) -> JobState {
    match arrival {
        Arrival::Live => JobState::Ready,
        Arrival::Fallback => JobState::Fallback,
    }
}

/// How long the hurt blink (and the invulnerability it grants) lasts.
pub const HURT_INVULN_S: f32 = 1.0;
/// How far a hit with a known source shoves the player away from it.
const KNOCKBACK_PX: f32 = 22.0;
/// The death animation plays this long before the Game Over panel shows.
pub const DEATH_ANIM_S: f32 = 1.2;

/// Stats for the current run, for the Game Over screen.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunStats {
    pub flasks_solved: u32,
    pub monsters_beaten: u32,
    /// Gears picked up and not yet spent (FR17). They reset with the run.
    pub gears: u32,
}

pub struct Game {
    pub player: Player,
    pub world: World,
    pub progression: Progression,
    pub trigger_state: TriggerState,
    pub input_buffer: String,
    /// Set when E is pressed with a task open; the task panel consumes it
    /// by focusing the answer field.
    pub focus_answer: bool,
    pub toast: Option<(String, f32)>,
    /// Paused with P (FR6). The game is frozen while this or
    /// `settings_open` is set; see [`Game::is_paused`].
    pub paused: bool,
    /// The settings modal is open (FR7). Kept apart from `paused` so
    /// closing the modal doesn't undo a P pause made before it opened.
    pub settings_open: bool,
    /// The field guide is open (FR22). Pauses the game like the settings do.
    pub guide_open: bool,
    /// Cards for the monsters beaten so far this session.
    pub guide: field_guide::FieldGuide,
    /// The discovery map is open (FR27). It freezes the world like the
    /// settings modal, and only opens when nothing else has the screen.
    pub map_open: bool,
    /// The shop at a safe stop is open (FR17, FR28). It freezes the world like
    /// the map does.
    pub shop_open: bool,
    /// What the player last tried to buy and why it was refused, shown in the
    /// shop until the next purchase.
    pub shop_note: Option<String>,
    /// The player is at a safe stop's cottage door: E opens the shop.
    stop_target: bool,
    /// The safe stop the player was last told about (FR28), by number, so its
    /// welcome toast plays once on arrival; 0 when outside any stop.
    in_stop: u32,
    /// Sparkle bursts where gears were picked up (FR17): world position and
    /// seconds since.
    sparks: Vec<(Vec2, f32)>,
    /// How far up the road the player has been (FR27).
    pub discovery: map::Discovery,
    pub settings: Settings,
    /// Player HP (FR13).
    pub health: Health,
    /// The doctor's curse (FR1): dormant until the player first steps onto
    /// the road.
    pub curse: Curse,
    pub run: RunStats,
    /// Best distance this session (not saved between launches).
    pub best_distance_m: u32,
    /// Seconds since death while the run is over (FR14); `None` while
    /// playing.
    pub game_over: Option<f32>,
    /// Screen-shake time left after a hit.
    shake: f32,
    /// The wild encounter being fought, if any (FR10).
    pub battle: Option<Battle>,
    /// The open NPC conversation, if any (FR9).
    pub dialogue: Option<dialogue::Dialogue>,
    /// NPC lines from `assets/dialogue.json`.
    dialogue_bank: Vec<dialogue::NpcLines>,
    /// Times the player has talked to Professor Pebble, to rotate lines.
    pebble_talks: usize,
    /// The player is close enough to Professor Pebble to talk.
    npc_in_reach: bool,
    /// "Item get" celebration after a solved flask (FR9): seconds in, and
    /// the banner text.
    item_get: Option<(f32, String)>,
    /// Set each frame from egui: a text field has keyboard focus, so key
    /// presses are typing, not game controls (P in the answer field).
    pub typing: bool,
    /// Game clock for animation; only advances while unpaused, so waves,
    /// lanterns and flasks freeze with the pause.
    time: f32,
    /// Flask that pressing E would open right now — drawn with a key
    /// prompt over it.
    interact_target: Option<usize>,
    /// Road-blocking monster (FR2) that pressing E would challenge right
    /// now: an index into `world.zones`, drawn with a key prompt over it.
    blocker_target: Option<usize>,
    client: CerebrasClient,
    fallback_tasks: Vec<MathTask>,
    /// World y at the top of the screen.
    cam_y: f32,
    /// Flask whose task is currently loading/open, so the result can be
    /// stamped onto it.
    active_spot: Option<usize>,
    /// Monsters defined by specs in the content dir (FR21), kept current
    /// by the watcher. Content, not run state: `reset` keeps it.
    pub roster: Roster,
    /// The content watcher's channel, drained every frame.
    content_rx: Option<Receiver<ContentEvent>>,
    /// The goalpost tracker at the top centre (FR31): what's next up the
    /// road and how far. Painted by `ui::goalpost_panel`.
    pub goalpost: Tracker,
    /// Questions asked for ahead of need, and the requests that fill them
    /// (FR32). Content, not run state: `reset` keeps it, since the questions
    /// aren't tied to a run and it makes the next run's first battle instant.
    pub prefetch: Prefetch,
    /// How long the player waited for each question shown (FR32).
    pub waits: WaitStats,
    /// What the model is being asked to make now, for the creation bar
    /// (FR33). A read-only mirror: nothing reads it to decide anything.
    pub activity: GenActivity,
    /// The mad scientist at the top of the road (FR23). Cosmetic: nothing a
    /// rule reads depends on them.
    pub villain: Villain,
    /// The question in `trigger_state` while it's still loading, for the wait
    /// measurement and the soft deadline.
    asking: Option<Ask>,
    /// A battle's next question, asked for while the attack animation plays and
    /// shown when the phase reaches `Question` (FR32).
    next_question: Option<(TriggerState, Ask)>,
    /// The background monster designer (FR21), if there's a key to ask with.
    studio: Option<Studio>,
    /// Generated species that have been put in front of the player, so the
    /// studio stocks up on new ones instead of what's already been seen.
    seen_generated: std::collections::HashSet<String>,
    /// The studio's finished count when last looked at, and the seconds left
    /// before it may be asked for another: a monster it just wrote isn't in
    /// the roster until the watcher's next scan, and counting it as missing
    /// would order a second one.
    studio_made_seen: u32,
    studio_bad_seen: u32,
    studio_settle: f32,
    /// Studio jobs on the creation bar: the studio's job number and its circle.
    studio_jobs: Vec<(u64, u32)>,
    /// The roster has been frozen into the world's pool for this run: spec
    /// monsters can stand in zones, and new files join only on the next run.
    roster_frozen: bool,
    /// How the last wrong answer works out, shown beside the question (FR22).
    pub explain: Option<Explain>,
    /// Lesson packs loaded and problems they gave the buffer this session.
    packs_loaded: u32,
    /// Lesson jobs asked for, to rotate topics.
    lesson_jobs: usize,
    /// Topics Professor Pebble has been asked for tips on this session.
    tips_asked: std::collections::HashSet<String>,
    /// Tips waiting for Pebble to say them, oldest first (FR22).
    pebble_tips: std::collections::VecDeque<String>,
}

/// A worked explanation of a problem the player got wrong (FR22): shown beside
/// the next question for a few seconds, long enough to read.
#[derive(Clone, Debug)]
pub struct Explain {
    pub problem: String,
    pub steps: Vec<String>,
    /// Seconds left on screen (game time).
    pub left: f32,
}

/// Seconds an explanation stays up: a few to start, and a while per step.
const EXPLAIN_BASE_S: f32 = 3.0;
const EXPLAIN_PER_STEP_S: f32 = 2.0;

/// How many unseen generated monsters of the tier coming up the studio keeps
/// ready. A zone never waits for one (FR32), so the stock has to be built
/// ahead of the player.
const STUDIO_STOCK: usize = 3;
/// What a battle's question calls itself on the creation bar.
const BATTLE_LABEL: &str = "battle";
/// Seconds the studio rests after a job the model got wrong.
const STUDIO_BACKOFF_S: f32 = 20.0;

fn camera_target(player_y: f32) -> f32 {
    player_y - screen_height() * CAMERA_ANCHOR
}

impl Game {
    pub fn new(client: CerebrasClient, fallback_tasks: Vec<MathTask>) -> Self {
        let start = vec2(
            world::path_center(world::PLAYER_START_Y),
            world::PLAYER_START_Y,
        );

        Self {
            player: Player::new(start),
            world: World::new(),
            progression: Progression::new(),
            trigger_state: TriggerState::Idle,
            input_buffer: String::new(),
            focus_answer: false,
            toast: None,
            paused: false,
            settings_open: false,
            guide_open: false,
            guide: Default::default(),
            map_open: false,
            shop_open: false,
            shop_note: None,
            stop_target: false,
            in_stop: 0,
            sparks: Vec::new(),
            discovery: map::Discovery::new(),
            settings: Settings::default(),
            health: Health::default(),
            curse: Curse::new(),
            run: RunStats::default(),
            best_distance_m: 0,
            game_over: None,
            shake: 0.0,
            battle: None,
            item_get: None,
            dialogue: None,
            dialogue_bank: dialogue::load_bank(),
            pebble_talks: 0,
            npc_in_reach: false,
            typing: false,
            time: 0.0,
            interact_target: None,
            blocker_target: None,
            client,
            fallback_tasks,
            cam_y: camera_target(start.y),
            active_spot: None,
            roster: Roster::default(),
            content_rx: None,
            goalpost: Tracker::new(),
            prefetch: Prefetch::new(PrefetchConfig::from_env()),
            waits: WaitStats::default(),
            activity: GenActivity::new(),
            villain: Villain::new(),
            asking: None,
            next_question: None,
            studio: None,
            seen_generated: Default::default(),
            studio_made_seen: 0,
            studio_bad_seen: 0,
            studio_settle: 0.0,
            studio_jobs: Vec::new(),
            roster_frozen: false,
            explain: None,
            packs_loaded: 0,
            lesson_jobs: 0,
            tips_asked: Default::default(),
            pebble_tips: Default::default(),
        }
    }

    /// Starts taking content events from a watcher
    /// (`content::watcher::spawn`). Without one the roster stays empty
    /// and only the built-in monsters exist.
    pub fn watch_content(&mut self, rx: Receiver<ContentEvent>) {
        self.content_rx = Some(rx);
    }

    /// Hands the game the studio that designs new monsters in the background
    /// (`content::studio::Studio::spawn`). Without one only the monsters
    /// already in the content dir exist.
    pub fn attach_studio(&mut self, studio: Option<Studio>) {
        self.studio = studio;
    }

    /// What the studio has done so far, for the token panel; `None` when
    /// there isn't one.
    pub fn studio_status(&self) -> Option<StudioStatus> {
        self.studio.as_ref().map(Studio::status)
    }

    /// Applies whatever the content watcher has found since last frame.
    /// Runs every frame, paused or not, so an edit shows even on a paused
    /// battle. A spec's look changes at once, also mid-battle; its rules
    /// only from its next battle (see `Battle::info`).
    fn poll_content(&mut self) {
        let Some(rx) = &self.content_rx else {
            return;
        };
        let mut events = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(e) => events.push(e),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.content_rx = None;
                    break;
                }
            }
        }
        let any = !events.is_empty();
        for event in events {
            self.roster.apply(&event);
            match &event {
                ContentEvent::Monster { spec, reloaded } => {
                    if let Some(b) = &mut self.battle {
                        b.respec(spec);
                    }
                    self.world.respec(spec);
                    if *reloaded {
                        self.toast = Some((format!("Reloaded {}", spec.name), 1.5));
                    }
                }
                ContentEvent::Invalid { file, error } => {
                    eprintln!("content: {file}: {error} (keeping the last good version)");
                    self.toast = Some((format!("{file}: {error}"), 4.0));
                }
                ContentEvent::Lessons { pack, .. } => {
                    // Verified problems join the question buffer, keyed by
                    // their level, and are used by `request_task`.
                    let added = pack
                        .tasks
                        .iter()
                        .filter(|t| self.prefetch.buffer.push_unique(pack.level, (*t).clone()))
                        .count();
                    if added > 0 {
                        self.packs_loaded += 1;
                    }
                }
                ContentEvent::Tips(tips) => {
                    if tips.npc == "Professor Pebble" {
                        for line in &tips.lines {
                            if !self.pebble_tips.contains(line) && self.pebble_tips.len() < 12 {
                                self.pebble_tips.push_back(line.clone());
                            }
                        }
                    }
                }
                ContentEvent::Removed { .. } => {}
            }
        }
        // The first scan reports every file already there. Once it has landed,
        // freeze the roster into the world: the run's zones may hold those
        // monsters. Files that appear later (the studio's) wait for the next run.
        if any && !self.roster_frozen {
            self.world.set_pool(self.roster_pool());
            self.roster_frozen = true;
        }
    }

    /// The roster as a pool for the world, sorted by id.
    fn roster_pool(&self) -> Vec<std::sync::Arc<crate::content::MonsterSpec>> {
        self.roster.iter().cloned().collect()
    }

    /// True while the world is frozen: paused with P or the settings
    /// modal is open.
    pub fn is_paused(&self) -> bool {
        self.paused || self.settings_open || self.guide_open || self.map_open || self.shop_open
    }

    /// Every field guide card, found or not, for the panel.
    pub fn guide_cards(&self) -> Vec<field_guide::GuideCard> {
        self.guide.cards(&self.roster)
    }

    /// The cloud model client, for the HUD's token-usage readout.
    pub fn llm(&self) -> &CerebrasClient {
        &self.client
    }

    /// Meters walked up the road from the gateway (0 in the hub), for the HUD.
    pub fn distance_m(&self) -> u32 {
        ((world::ROAD_START_Y - self.player.pos.y).max(0.0) / world::PX_PER_METER) as u32
    }

    /// The map opens from the road only: not during a flask problem, a
    /// battle, a dialogue or while settings are up.
    fn can_open_map(&self) -> bool {
        !self.settings_open
            && !self.guide_open
            && !self.shop_open
            && self.battle.is_none()
            && self.dialogue.is_none()
            && matches!(
                self.trigger_state,
                TriggerState::Idle | TriggerState::Resolved { .. }
            )
    }

    /// What the map panel draws this frame.
    pub fn map_view(&self) -> map::MapView {
        self.discovery
            .snapshot(&self.world, self.player.pos.y, self.player.pos.x)
    }

    pub fn update(&mut self, dt: f32) {
        self.poll_content();
        self.activity.tick(dt);
        self.sync_creation_bar();
        if let Some(t) = &mut self.game_over {
            // Only the restart key works once the run is over.
            *t += dt;
            self.player.tick_death(dt);
            if self.can_restart()
                && (is_key_pressed(INTERACT_KEY) || is_key_pressed(KeyCode::Enter))
            {
                self.reset();
            }
            return;
        }

        if !self.typing {
            if self.shop_open {
                if is_key_pressed(KeyCode::Escape) || is_key_pressed(INTERACT_KEY) {
                    self.shop_open = false;
                }
            } else if self.map_open {
                if is_key_pressed(MAP_KEY) || is_key_pressed(KeyCode::Escape) {
                    self.map_open = false;
                }
            } else if is_key_pressed(MAP_KEY) && self.can_open_map() {
                self.map_open = true;
            } else if is_key_pressed(SETTINGS_KEY)
                || (self.settings_open && is_key_pressed(KeyCode::Escape))
            {
                self.settings_open = !self.settings_open;
                self.guide_open = false;
            } else if !self.settings_open
                && (is_key_pressed(GUIDE_KEY)
                    || (self.guide_open && is_key_pressed(KeyCode::Escape)))
            {
                self.guide_open = !self.guide_open;
            } else if !self.settings_open && !self.guide_open && is_key_pressed(PAUSE_KEY) {
                self.paused = !self.paused;
            }
        }
        if self.is_paused() {
            // Keep polling so a Cerebras response that lands mid-pause is
            // stored, not lost; nothing else advances.
            self.poll_questions(0.0);
            self.set_studio_gate();
            return;
        }
        self.time += dt;
        self.shake = (self.shake - dt).max(0.0);
        if let Some((_, remaining)) = &mut self.toast {
            *remaining -= dt;
            if *remaining <= 0.0 {
                self.toast = None;
            }
        }
        if let Some(e) = &mut self.explain {
            e.left -= dt;
            if e.left <= 0.0 {
                self.explain = None;
            }
        }
        if let Some((t, _)) = &mut self.item_get {
            *t += dt;
            if *t > player::CHEER_S + 0.4 {
                self.item_get = None;
            }
        }
        self.sparks.retain_mut(|(_, age)| {
            *age += dt;
            *age < 0.9
        });
        self.tick_goalpost(dt);
        self.watch_villain();
        // The studio works in battles and dialogues too, behind the questions.
        self.tick_studio(dt);
        if self.battle.is_some() {
            self.update_battle(dt);
            return;
        }
        if let Some(d) = &mut self.dialogue {
            // Held still while talking; E types out, advances, closes.
            d.update(dt);
            self.player.stop(dt);
            if is_key_pressed(INTERACT_KEY) {
                self.advance_dialogue();
            }
            return;
        }

        // The player stops at a flask while its problem is loading or
        // open: each flask is a checkpoint, and a fast model means a
        // short stop.
        let busy = matches!(
            self.trigger_state,
            TriggerState::Loading(_) | TriggerState::Active(_)
        );
        if busy {
            self.player.stop(dt);
        } else {
            // The road, and past it nothing: an undefeated monster on the
            // road (FR2) shuts it.
            let speed = self.progression.speed_multiplier() * self.curse.speed_factor();
            let w = &self.world;
            self.player
                .update(dt, speed, |p| w.clamp_blocked(world::clamp_to_path(p)));
            // The curse holds still while a question is loading or open
            // (owner, FR1), like the player.
            self.tick_curse(dt);
            // Hearts mend on their own while the player is free to roam,
            // unless the fever is at full (invariant 6: through `heal`).
            // At a safe stop they mend several times faster (FR28).
            let rate = if world::in_safe_stop(self.player.pos.y) {
                health::REST_REGEN_RATE
            } else {
                1.0
            };
            self.health.tick_rate(dt, self.curse.level < 1.0, rate);
            if self.game_over.is_some() {
                return;
            }
            self.pick_up_gears();
            self.watch_stop();
        }
        self.discovery.note(self.player.pos.y);

        let target = camera_target(self.player.pos.y);
        self.cam_y += (target - self.cam_y) * (1.0 - (-dt * CAMERA_FOLLOW).exp());
        self.world.ensure_generated(self.cam_y - world::CHUNK_H);
        self.villain
            .update(dt, self.cam_y, screen_height(), self.player.pos);

        self.poll_questions(dt);
        self.tick_prefetch();

        // Stepping into a monster's patch of ferns starts a battle.
        if !busy {
            if let Some(z) = self.world.encounter_at(self.player.pos) {
                self.start_battle(self.world.zones[z].species(), Some(z));
                return;
            }
        }

        let idle = matches!(self.trigger_state, TriggerState::Idle);
        self.interact_target = match idle {
            true => self.world.interactable_spot(self.player.pos),
            false => None,
        };
        self.blocker_target = match idle && self.interact_target.is_none() {
            true => self.world.blocker_in_reach(self.player.pos),
            false => None,
        };
        self.stop_target = idle
            && self.interact_target.is_none()
            && self.blocker_target.is_none()
            && self.world.stop_in_reach(self.player.pos).is_some();
        self.npc_in_reach = idle
            && self.interact_target.is_none()
            && self.blocker_target.is_none()
            && !self.stop_target
            && self.player.pos.distance(world::pebble_pos()) < world::NPC_TALK_DIST;

        if is_key_pressed(INTERACT_KEY) {
            match (&self.trigger_state, self.interact_target) {
                (TriggerState::Idle, Some(i)) => self.open_spot(i),
                (TriggerState::Idle, None) if self.blocker_target.is_some() => {
                    self.challenge_blocker()
                }
                (TriggerState::Idle, None) if self.stop_target => self.open_shop(),
                (TriggerState::Idle, None) if self.npc_in_reach => self.talk_to_pebble(),
                (TriggerState::Active(_), _) => self.focus_answer = true,
                _ => {}
            }
        }

        if let TriggerState::Resolved { cooldown, .. } = &mut self.trigger_state {
            *cooldown -= dt;
            if *cooldown <= 0.0 {
                self.trigger_state = TriggerState::Idle;
            }
        }
    }

    /// Picks up the gears the player is standing on (FR17): counted in the run,
    /// each with a small sparkle where it lay.
    fn pick_up_gears(&mut self) {
        let got = self.world.collect_gears(self.player.pos);
        self.run.gears += got.len() as u32;
        self.sparks.extend(got.into_iter().map(|at| (at, 0.0)));
    }

    /// Greets the player the first time they step into a safe stop (FR28).
    fn watch_stop(&mut self) {
        let y = self.player.pos.y;
        match world::stop_at(y).filter(|_| world::in_safe_stop(y)) {
            Some(s) if s.n != self.in_stop => {
                self.in_stop = s.n;
                self.toast = Some((
                    "A safe stop. The fever lifts and your hearts mend. E at the cottage: shop."
                        .into(),
                    4.5,
                ));
            }
            Some(_) => {}
            None => self.in_stop = 0,
        }
    }

    /// Opens the stop's shop (FR17, FR28).
    fn open_shop(&mut self) {
        self.shop_open = true;
        self.shop_note = None;
    }

    /// Buys `item` with the run's gears (FR17): the one place a purchase
    /// happens. A refusal is shown in the shop and changes nothing.
    pub fn buy(&mut self, item: shop::Item) {
        self.shop_note = match shop::buy(item, &mut self.run.gears, &mut self.health) {
            Ok(()) => Some(format!("{}: bought.", item.name())),
            Err(why) => Some(format!("{}.", why.reason())),
        };
    }

    /// Advances the goalpost tracker (FR31). It steps aside in a battle, a
    /// dialogue, while a flask's problem is loading or open, in the hub
    /// (nothing is placed there) and when the HUD is hidden.
    fn tick_goalpost(&mut self, dt: f32) {
        let y = self.player.pos.y;
        let busy = matches!(
            self.trigger_state,
            TriggerState::Loading(_) | TriggerState::Active(_)
        );
        let visible = self.settings.show_hud
            && self.battle.is_none()
            && self.dialogue.is_none()
            && !busy
            && !world::in_hub(y);
        let target = if visible {
            self.world.next_goalpost(y)
        } else {
            None
        };
        self.goalpost.update(dt, y, target, visible);
    }

    /// Tells the doctor how the run is going, so they can remark on a level, a
    /// streak, a milestone or low hearts (FR23). The one-off events (an answer, a
    /// hit, a battle) cue them where they happen.
    fn watch_villain(&mut self) {
        self.villain.observe(villain::Snapshot {
            level: self.progression.level,
            streak: self.progression.streak,
            meters: self.distance_m(),
            health: self.health.current,
        });
    }

    /// Advances the curse (FR1) and acts on what it reports: a toast as it
    /// takes hold or worsens, half a heart at full.
    fn tick_curse(&mut self, dt: f32) {
        use curse::{Stage, Tick};
        let line = match self.curse.tick(dt, self.player.pos.y) {
            Tick::Nothing => return,
            Tick::Started => "The doctor's curse takes hold... keep moving!",
            Tick::Worsened(Stage::Clear) => return,
            Tick::Worsened(Stage::Cursed) => "A fever creeps in. New ground holds it back.",
            Tick::Worsened(Stage::Fever) => "The fever slows you. Press on, or solve a flask!",
            Tick::Worsened(Stage::Burning) => "The fever burns! You're losing hearts.",
            Tick::Drain => {
                self.damage_player(1, None);
                return;
            }
        };
        self.toast = Some((line.into(), 3.0));
    }

    /// Opens Professor Pebble's conversation: the hub tutorial, then one of
    /// their lines from the bank, a different one each time.
    fn talk_to_pebble(&mut self) {
        let mut lines: Vec<String> = dialogue::PEBBLE_TUTORIAL
            .iter()
            .map(|s| s.to_string())
            .collect();
        // A tip written for what the player has been missing comes before the
        // usual encouragement (FR22).
        if let Some(tip) = self.pebble_tips.pop_front() {
            lines.push(tip);
        } else if let Some(bank) = self
            .dialogue_bank
            .iter()
            .find(|n| n.npc == "Professor Pebble")
        {
            if !bank.lines.is_empty() {
                lines.push(bank.lines[self.pebble_talks % bank.lines.len()].clone());
            }
        }
        self.pebble_talks += 1;
        self.npc_in_reach = false;
        self.dialogue = Some(dialogue::Dialogue::new("Professor Pebble", lines));
    }

    fn advance_dialogue(&mut self) {
        if let Some(d) = &mut self.dialogue {
            if !d.advance() {
                self.dialogue = None;
            }
        }
    }

    /// Fights the road-blocking monster in reach (FR2); beating it clears
    /// the road.
    fn challenge_blocker(&mut self) {
        if let Some(z) = self.blocker_target.take() {
            self.start_battle(self.world.zones[z].species(), Some(z));
        }
    }

    /// Starts a battle with `species`. `zone` is the encounter zone to
    /// clear on victory (`None` for a scripted dev battle).
    fn start_battle(&mut self, species: Species, zone: Option<usize>) {
        self.stash_questions();
        // The first time a studio-made species is met gets a banner.
        let new_species = matches!(&species, Species::Spec(s)
            if studio::is_generated(&s.id) && !self.seen_generated.contains(&s.id));
        if let Species::Spec(spec) = &species {
            self.seen_generated.insert(spec.id.clone());
        }
        self.villain.cue(VillainCue::BattleStart);
        self.trigger_state = TriggerState::Idle;
        self.interact_target = None;
        self.blocker_target = None;
        self.input_buffer.clear();
        self.player.action = player::Action::None;
        // The battle view puts the monster on the right.
        self.player.facing = 1.0;
        // Ask for the first question now: the entry transition and the intro
        // (1.8 s) cover the round trip. It's shown when the intro ends.
        let bias = species.info().difficulty_bias;
        let topic = species.topic();
        let mut battle = Battle::new(species, zone.unwrap_or(usize::MAX));
        battle.new_species = new_species;
        self.battle = Some(battle);
        self.next_question = Some(self.request_task(bias, prefetch::SOFT_DEADLINE_BATTLE_S, topic));
    }

    /// Advances the battle's phases: transition, intro, attacks, faint.
    /// The player is held still throughout.
    fn update_battle(&mut self, dt: f32) {
        self.player.stop(dt);
        self.poll_questions(dt);
        if is_key_pressed(INTERACT_KEY) && matches!(self.trigger_state, TriggerState::Active(_)) {
            self.focus_answer = true;
        }
        let Some(b) = self.battle.as_mut() else {
            return;
        };

        let (mut next_question, mut monster_hit, mut won, mut done) = (false, None, false, false);
        b.phase = match b.phase {
            Phase::Enter(t) if t + dt >= battle::ENTER_S => Phase::Intro(0.0),
            Phase::Enter(t) => Phase::Enter(t + dt),
            Phase::Intro(t) if t + dt >= battle::INTRO_S => {
                next_question = true;
                Phase::Question
            }
            Phase::Intro(t) => Phase::Intro(t + dt),
            Phase::Question => Phase::Question,
            Phase::PlayerAttack(t) => {
                let t = t + dt;
                if t >= battle::PLAYER_HIT_AT && !b.hit_landed {
                    b.hit_landed = true;
                    b.hits_left = b.hits_left.saturating_sub(1);
                }
                if t < battle::PLAYER_ATTACK_S {
                    Phase::PlayerAttack(t)
                } else if b.hits_left == 0 {
                    Phase::Faint(0.0)
                } else {
                    next_question = true;
                    Phase::Question
                }
            }
            Phase::MonsterAttack(t) => {
                let t = t + dt;
                if t >= battle::MONSTER_HIT_AT && !b.hit_landed {
                    b.hit_landed = true;
                    monster_hit = Some((b.damage(), b.info.stun_s, b.info.steals_hat));
                    b.misses += 1;
                }
                if t < battle::MONSTER_ATTACK_S {
                    Phase::MonsterAttack(t)
                } else {
                    next_question = true;
                    Phase::Question
                }
            }
            Phase::Faint(t) if t + dt >= battle::FAINT_S => {
                won = true;
                Phase::Exit(0.0)
            }
            Phase::Faint(t) => Phase::Faint(t + dt),
            Phase::Exit(t) if t + dt >= battle::EXIT_S => {
                done = true;
                Phase::Exit(battle::EXIT_S)
            }
            Phase::Exit(t) => Phase::Exit(t + dt),
        };
        let (info, zone) = (b.info, b.zone);
        let name = b.species.name().to_string();
        let topic = b.species.topic();
        let species = b.species.clone();

        if let Some((halves, stun_s, steals_hat)) = monster_hit {
            self.damage_player(halves, None);
            if steals_hat && !self.player.hat_stolen {
                self.player.hat_stolen = true;
                self.toast = Some((format!("The {name} swiped your hat!"), 2.0));
            }
            if stun_s > 0.0 && self.game_over.is_none() {
                self.player.stun(stun_s);
            }
        }
        if next_question && self.game_over.is_none() {
            // Usually already asked for, or even back (see `battle_answer`).
            let asked = self.next_question.take().unwrap_or_else(|| {
                self.request_task(
                    info.difficulty_bias,
                    prefetch::SOFT_DEADLINE_BATTLE_S,
                    topic,
                )
            });
            self.show_question(asked);
        }
        if won {
            if self.player.hat_stolen {
                self.player.hat_stolen = false;
            }
            let mut opened = false;
            if let Some(z) = self.world.zones.get_mut(zone) {
                z.cleared = true;
                opened = z.blocks;
            }
            self.run.monsters_beaten += 1;
            self.villain.cue(VillainCue::BattleWon);
            self.stash_questions();
            self.trigger_state = TriggerState::Idle;
            // Beating a monster unlocks its field guide card (FR22).
            let card = if self.guide.unlock(&species) {
                " New field guide card!"
            } else {
                ""
            };
            let line = match opened {
                true => format!("You beat the {name}! The road is open.{card}"),
                false => format!("You beat the {name}!{card}"),
            };
            self.toast = Some((line, 2.5));
        }
        if done {
            self.battle = None;
            self.stash_questions();
            self.trigger_state = TriggerState::Idle;
        }
    }

    /// Applies an answer to the battle's open question: a right one lands
    /// the player's attack, a wrong one lets the monster attack.
    fn battle_answer(&mut self, correct: bool) {
        let Some(b) = self.battle.as_mut() else {
            return;
        };
        if b.phase != Phase::Question {
            return;
        }
        b.hit_landed = false;
        let xp = b.info.xp_per_hit;
        let bias = b.info.difficulty_bias;
        let topic = b.species.topic();
        // The answer that lands the last hit leaves nothing to ask.
        let last_hit = correct && b.hits_left <= 1;
        if correct {
            b.phase = Phase::PlayerAttack(0.0);
            self.player.shoot_at_monster();
            let level = self.progression.level;
            for upgrade in self.progression.record_correct(xp) {
                self.toast = Some((format!("Unlocked: {}", upgrade.name), 3.0));
            }
            if self.progression.level > level {
                self.health.refill();
            }
        } else {
            b.phase = Phase::MonsterAttack(0.0);
            self.progression.record_incorrect();
        }
        self.trigger_state = TriggerState::Resolved {
            correct,
            cooldown: f32::MAX,
        };
        self.input_buffer.clear();
        // Ask for the next question now, so the attack that plays next (0.8 s
        // or 0.9 s) covers the round trip, instead of starting the request
        // once the attack is over.
        if !last_hit && self.next_question.is_none() {
            self.next_question =
                Some(self.request_task(bias, prefetch::SOFT_DEADLINE_BATTLE_S, topic));
        }
    }

    /// True once the death animation has played and the Game Over panel
    /// is up.
    pub fn can_restart(&self) -> bool {
        matches!(self.game_over, Some(t) if t >= DEATH_ANIM_S)
    }

    /// The one entry point for hurting the player (FR13). `from` is where
    /// the hit came from, for the knockback; `None` means no direction
    /// (e.g. a wrong answer). Returns false if the hit was ignored because
    /// the player is invulnerable or already dead.
    pub fn damage_player(&mut self, halves: u32, from: Option<Vec2>) -> bool {
        if self.game_over.is_some() || self.player.flicker.is_some_and(|f| f.active()) {
            return false;
        }
        // Nothing hurts the player inside a safe stop (FR28).
        if world::in_safe_stop(self.player.pos.y) {
            return false;
        }
        self.health.damage(halves);
        if self.health.is_dead() {
            self.die();
            return true;
        }
        self.villain.cue(VillainCue::PlayerHurt);
        self.player
            .start_flicker(FlickerKind::Damage, HURT_INVULN_S);
        if let Some(src) = from {
            let away = (self.player.pos - src).normalize_or(vec2(0.0, 1.0));
            let pushed = world::clamp_to_path(self.player.pos + away * KNOCKBACK_PX);
            self.player.pos = self.world.clamp_blocked(pushed);
        }
        if !self.settings.reduce_flashing {
            self.shake = 0.25;
        }
        true
    }

    fn die(&mut self) {
        self.player.die();
        self.game_over = Some(0.0);
        self.paused = false;
        self.settings_open = false;
        self.map_open = false;
        self.shop_open = false;
        self.best_distance_m = self.best_distance_m.max(self.distance_m());
        if let Some(i) = self.active_spot.take() {
            self.world.spots[i].state = SpotState::Missed;
        }
    }

    /// Starts a new run from the start (FR14) without restarting the app:
    /// keeps the model client, fallback bank, settings and the session's
    /// best distance; everything else is fresh. Dropping the trigger state
    /// discards any in-flight task request.
    pub fn reset(&mut self) {
        let start = vec2(
            world::path_center(world::PLAYER_START_Y),
            world::PLAYER_START_Y,
        );
        self.stash_questions();
        self.activity.clear();
        self.studio_jobs.clear();
        self.player = Player::new(start);
        // A new run takes a new snapshot of the roster, so monsters the studio
        // wrote during the last one can now appear.
        self.world = World::with_pool(self.roster_pool());
        self.progression = Progression::new();
        self.trigger_state = TriggerState::Idle;
        self.input_buffer.clear();
        self.focus_answer = false;
        self.toast = None;
        self.paused = false;
        self.settings_open = false;
        self.guide_open = false;
        self.map_open = false;
        self.shop_open = false;
        self.shop_note = None;
        self.stop_target = false;
        self.in_stop = 0;
        self.sparks.clear();
        self.discovery = map::Discovery::new();
        self.health = Health::default();
        self.curse = Curse::new();
        self.run = RunStats::default();
        self.game_over = None;
        self.shake = 0.0;
        self.interact_target = None;
        self.blocker_target = None;
        self.cam_y = camera_target(start.y);
        self.active_spot = None;
        self.battle = None;
        self.item_get = None;
        self.dialogue = None;
        self.npc_in_reach = false;
        self.goalpost = Tracker::new();
        self.explain = None;
        self.villain.reset();
    }

    /// One line of run state, printed by the `JEKO_SHOT` hook so scripted
    /// runs can be checked without a screenshot.
    pub fn debug_summary(&self) -> String {
        let trigger = match &self.trigger_state {
            TriggerState::Idle => "idle",
            TriggerState::Loading(_) => "loading",
            TriggerState::Active(_) => "active",
            TriggerState::Resolved { .. } => "resolved",
        };
        format!(
            "STATE hp={}/{} level={} xp={} streak={} dist={}m flasks={} beaten={} \
             zones_cleared={} battle={:?} trigger={} dialogue={} game_over={} hat_stolen={} \
             arrows={} gears={} stop={} roster={} spec_zones={} curse={:.2} blocker_in_reach={} blockers_left={} goalpost={} \
             questions=[{}] buffered={:?} flying={} packs_loaded={} pebble_tips={} weakest={} explain={} studio={} villain={}",
            self.health.current,
            self.health.max,
            self.progression.level,
            self.progression.lifetime_xp,
            self.progression.streak,
            self.distance_m(),
            self.run.flasks_solved,
            self.run.monsters_beaten,
            self.world.zones.iter().filter(|z| z.cleared).count(),
            self.battle
                .as_ref()
                .map(|b| (b.species.name().to_string(), b.hits_left, b.phase)),
            trigger,
            self.dialogue.is_some(),
            self.game_over.is_some(),
            self.player.hat_stolen,
            self.player.arrows.len(),
            self.run.gears,
            self.in_stop,
            self.roster.len(),
            self.world.zones.iter().filter(|z| z.spec.is_some()).count(),
            self.curse.level,
            self.blocker_target.is_some(),
            self.world
                .zones
                .iter()
                .filter(|z| z.blocks && !z.cleared)
                .count(),
            self.goalpost
                .view()
                .map(|v| format!("{} {}m open={:.1}", v.label, v.meters, v.open))
                .unwrap_or_else(|| "none".into()),
            self.waits
                .readout()
                .unwrap_or_else(|| "none shown yet".into()),
            self.prefetch.buffer.counts(),
            self.prefetch.in_flight(),
            self.packs_loaded,
            self.pebble_tips.len(),
            self.progression.weakest_topic().unwrap_or("none"),
            self.explain
                .as_ref()
                .map_or(0, |e| e.steps.len()),
            self.studio_status().map_or_else(
                || "off".to_string(),
                |s| format!(
                    "made={} queued={} working={} rejected={} failed={}",
                    s.made, s.queued, s.working, s.rejected, s.failed
                ),
            ),
            if self.villain.is_here() {
                format!(
                    "{:?} saying={:?}",
                    self.villain.action(),
                    self.villain.saying().unwrap_or_default()
                )
            } else {
                "away".to_string()
            },
        )
    }

    /// Queues a fake creation-bar job (`JEKO_DEV=gen:<kind>`), so each state
    /// can be checked by screenshot with no model.
    fn dev_gen(&mut self, what: &str) {
        let (kind, secs, end) = match what {
            "question" => (RequestKind::Question, 2.5, JobState::Ready),
            "prefetch" => (RequestKind::Prefetch, 2.5, JobState::Ready),
            "monster" => (RequestKind::Monster, 2.5, JobState::Ready),
            "lesson" => (RequestKind::Lesson, 2.5, JobState::Ready),
            "line" => (RequestKind::Line, 2.5, JobState::Ready),
            "fallback" => (RequestKind::Question, 2.5, JobState::Fallback),
            "slow" => (RequestKind::Question, 10.0, JobState::Ready),
            _ => return eprintln!("JEKO_DEV: unknown gen kind {what}"),
        };
        self.activity.scripted(kind, what, secs, end);
    }

    /// Whether the player is in the hub, before the gate, where nothing is
    /// being generated and the creation bar stays hidden.
    pub fn in_hub(&self) -> bool {
        world::in_hub(self.player.pos.y)
    }

    /// The creation bar's circles (FR33), for the UI to draw.
    /// In a battle only the battle's own question shows, so a wait never
    /// looks like a stall; the rest comes back after it.
    pub fn creation_view(&self) -> BarView {
        if self.battle.is_some() {
            self.activity
                .view_where(|j| matches!(j.kind, RequestKind::Question) && j.label == BATTLE_LABEL)
        } else {
            self.activity.view()
        }
    }

    /// Scripted events for the `JEKO_DEV` screenshot hook in `main.rs`.
    pub fn dev_event(&mut self, name: &str) {
        match name {
            "hurt" => {
                let from = self.player.pos + vec2(0.0, -30.0);
                self.damage_player(2, Some(from));
            }
            "kill" => {
                self.damage_player(self.health.current, None);
            }
            "jump" => self.player.action = player::Action::Jump { t: 0.0 },
            "roll" => {
                self.player.action = player::Action::Roll {
                    t: 0.0,
                    dir: self.player.facing,
                }
            }
            "shoot" => self.player.shoot(),
            // Spends every roll charge and the stamina (FR26), for a shot
            // of the skill bar recovering.
            "spent" => {
                while self.player.skills.roll.try_use() {}
                self.player.skills.stamina.exhaust();
            }
            "restart" => self.reset(),
            "pinch" => self.start_battle(Species::Builtin(MonsterKind::Pinchling), None),
            "jelly" => self.start_battle(Species::Builtin(MonsterKind::Jellyglow), None),
            "puff" => self.start_battle(Species::Builtin(MonsterKind::Puffspike), None),
            "coco" => self.start_battle(Species::Builtin(MonsterKind::CocoKnight), None),
            "gull" => self.start_battle(Species::Builtin(MonsterKind::GullBandit), None),
            "wyrm" => self.start_battle(Species::Builtin(MonsterKind::DuneWyrm), None),
            "titan" => self.start_battle(Species::Builtin(MonsterKind::TideTitan), None),
            // Fight a spec monster from the content dir (FR21) by its id.
            _ if name.starts_with("battle:") => {
                let id = &name["battle:".len()..];
                match self.roster.get(id) {
                    Some(spec) => self.start_battle(Species::Spec(spec.clone()), None),
                    None => eprintln!("JEKO_DEV: no monster {id:?} in the content dir"),
                }
            }
            // Answer the open question through the real submit path.
            "right" | "wrong" => {
                if let TriggerState::Active(task) = &self.trigger_state {
                    let a = task.answer + if name == "right" { 0.0 } else { 1.0 };
                    self.input_buffer = a.to_string();
                    self.submit_answer();
                }
            }
            // Teleports for scripted end-to-end runs; the next frame's
            // update then reacts as if the player had walked there.
            "tozone" => {
                self.world.ensure_generated(-20_000.0);
                if let Some(z) = self.world.zones.iter().find(|z| !z.cleared && !z.blocks) {
                    self.player.pos = z.pos + vec2(0.0, 10.0);
                    self.cam_y = camera_target(self.player.pos.y);
                }
            }
            // To just short of the first undefeated road blocker (FR2), at
            // the line it holds.
            "toblocker" => {
                self.world.ensure_generated(-20_000.0);
                if let Some(z) = self.world.zones.iter().find(|z| !z.cleared && z.blocks) {
                    self.player.pos = z.pos + vec2(0.0, world::BLOCK_GAP + 4.0);
                    self.cam_y = camera_target(self.player.pos.y);
                }
            }
            // To the first undefeated zone that holds a spec monster (FR21):
            // into its ferns, or to the line in front of it if it blocks.
            // `tospecblock` only counts road blockers.
            "tospec" | "tospecblock" => {
                self.world.ensure_generated(-20_000.0);
                let only_blockers = name == "tospecblock";
                if let Some(z) = self
                    .world
                    .zones
                    .iter()
                    .find(|z| !z.cleared && z.spec.is_some() && (z.blocks || !only_blockers))
                {
                    let dy = if z.blocks {
                        world::BLOCK_GAP + 4.0
                    } else {
                        10.0
                    };
                    self.player.pos = z.pos + vec2(0.0, dy);
                    self.cam_y = camera_target(self.player.pos.y);
                }
            }
            // Onto the road, 40 m up from the gate, for the goalpost tracker.
            "toroad" => {
                let y = world::ROAD_START_Y - 40.0 * world::PX_PER_METER;
                self.player.pos = vec2(world::path_center(y), y);
                self.cam_y = camera_target(y);
            }
            // To 8 m short of the next goalpost, to see the pulse close up.
            "neargoal" => {
                if let Some(g) = self.world.next_goalpost(self.player.pos.y) {
                    let y = g.y + 8.0 * world::PX_PER_METER;
                    self.player.pos = vec2(world::path_center(y), y);
                    self.cam_y = camera_target(y);
                }
            }
            // Onto the next goalpost, for the "Now!" state.
            "atgoal" => {
                if let Some(g) = self.world.next_goalpost(self.player.pos.y) {
                    let y = g.y - 1.0;
                    self.player.pos = vec2(world::path_center(y), y);
                    self.cam_y = camera_target(y);
                }
            }
            "toflask" => {
                if let Some(sp) = self
                    .world
                    .spots
                    .iter()
                    .find(|s| s.state == SpotState::Fresh)
                {
                    self.player.pos = world::clamp_to_path(sp.pos + vec2(0.0, 30.0));
                    self.cam_y = camera_target(self.player.pos.y);
                }
            }
            "topebble" => {
                self.player.pos = world::pebble_pos() + vec2(30.0, 10.0);
            }
            // Presses E: whatever E would do right now.
            "e" => match (&self.trigger_state, self.interact_target) {
                _ if self.can_restart() => self.reset(),
                _ if self.dialogue.is_some() => self.advance_dialogue(),
                (TriggerState::Idle, Some(i)) => self.open_spot(i),
                (TriggerState::Idle, None) if self.blocker_target.is_some() => {
                    self.challenge_blocker()
                }
                (TriggerState::Idle, None) if self.stop_target => self.open_shop(),
                (TriggerState::Idle, None) if self.npc_in_reach => self.talk_to_pebble(),
                _ => {}
            },
            // Ask the studio for a tier 2 monster now, whatever the stock.
            "genmonster" => self.enqueue_monster(2),
            "stun" => self.player.stun(2.5),
            "cheer" => {
                self.player.cheer();
                self.item_get = Some((0.0, "+20 XP!".into()));
            }
            "pause" => self.paused = true,
            "settings" => self.settings_open = true,
            "guide" => self.guide_open = true,
            // Win a card without the fight: every built-in, for a full-page shot.
            "cards" => {
                for k in MonsterKind::ALL.iter().take(4) {
                    self.guide.unlock(&Species::Builtin(*k));
                }
            }
            "map" => self.map_open = true,
            // Beside the first safe stop (FR28), in front of its cottage, with the
            // camera snapped. `shop` opens the shop; `gears:N` adds gears (FR17).
            "tostop" => {
                self.world.ensure_generated(-20_000.0);
                if let Some(s) = self.world.stops.first().copied() {
                    self.player.pos = world::clamp_to_path(s.door() + vec2(0.0, 24.0));
                    self.discovery.note(self.player.pos.y);
                    self.cam_y = camera_target(self.player.pos.y);
                }
            }
            // Just south of the first stop, on the road, to see it open up.
            "nearstop" => {
                self.world.ensure_generated(-20_000.0);
                if let Some(s) = self.world.stops.first().copied() {
                    let y = s.y + world::STOP_HALF_LEN + 150.0;
                    self.player.pos = vec2(world::path_center(y), y);
                    self.discovery.note(y);
                    self.cam_y = camera_target(y);
                }
            }
            "shop" => self.open_shop(),
            // The first trail of gears, close up.
            "togears" => {
                self.world.ensure_generated(-20_000.0);
                if let Some(g) = self.world.gears.iter().find(|g| !g.taken).map(|g| g.pos) {
                    self.player.pos = world::clamp_to_path(g + vec2(0.0, 170.0));
                    self.discovery.note(self.player.pos.y);
                    self.cam_y = camera_target(self.player.pos.y);
                }
            }
            n if n.starts_with("gears:") => match n["gears:".len()..].parse::<u32>() {
                Ok(count) => self.run.gears += count,
                Err(_) => eprintln!("JEKO_DEV: {n:?} needs a number"),
            },
            // 120 m up the road, discovered, for a shot of a filled-in map.
            "far" => {
                let y = world::ROAD_START_Y - 120.0 * world::PX_PER_METER;
                self.world.ensure_generated(y - world::CHUNK_H);
                self.player.pos = vec2(world::path_center(y), y);
                self.discovery.note(y);
            }
            "calm" => self.settings.reduce_flashing = true,
            // 40 m up the road with the camera snapped, so the doctor (FR23) is
            // on screen; they greet the player and then go about their business.
            "tovillain" => {
                let y = world::ROAD_START_Y - 40.0 * world::PX_PER_METER;
                self.world.ensure_generated(y - world::CHUNK_H);
                self.player.pos = vec2(world::path_center(y), y);
                self.discovery.note(y);
                self.cam_y = camera_target(y);
                self.villain
                    .update(0.0, self.cam_y, screen_height(), self.player.pos);
            }
            // The doctor reacts to a cue at once: `villain:right`, `:wrong`,
            // `:hurt`, `:streak`, `:levelup`, `:battle_won`, `:idle`, and so on
            // (`VillainCue::from_dev`). Needs them out on the road first.
            n if n.starts_with("villain:") => match VillainCue::from_dev(&n["villain:".len()..]) {
                Some(cue) => self.villain.cue_now(cue),
                None => eprintln!("JEKO_DEV: no villain cue {n:?}"),
            },
            // A fake job on the creation bar (FR33): `gen:question`,
            // `gen:monster`, `gen:lesson`, `gen:fallback`, `gen:slow` (writes
            // for ten seconds). Writes for 2.5 s, then is ready.
            n if n.starts_with("gen:") => self.dev_gen(&n["gen:".len()..]),
            // The curse (FR1) at the fever stage, and at full.
            "curse" => self.curse.force(0.75),
            "cursefull" => self.curse.force(1.0),
            // Hides the side panels, so a screenshot shows the whole world.
            "nohud" => {
                self.settings.show_hud = false;
                self.settings.show_minimap = false;
                self.settings.show_token_panel = false;
            }
            _ => eprintln!("JEKO_DEV: unknown event {name}"),
        }
    }

    fn open_spot(&mut self, i: usize) {
        self.world.spots[i].state = SpotState::Pending;
        self.active_spot = Some(i);
        self.interact_target = None;
        let asked = self.request_task(0, prefetch::SOFT_DEADLINE_FLASK_S, None);
        self.show_question(asked);
    }

    /// A fresh question, the only way one starts (invariant 6): a ready one
    /// from the buffer if there is one, otherwise live from the model if
    /// there's a key, otherwise from the fallback bank. `bias` raises the
    /// difficulty (tough monsters ask harder questions), and `topic` themes
    /// it (a spec monster's theme, FR22): only a question on that topic will do. `deadline_s` is how
    /// long a live call may take before a bank question is shown instead.
    /// The result goes to `show_question`, now or when it's needed.
    fn request_task(
        &mut self,
        bias: u32,
        deadline_s: f32,
        topic: Option<String>,
    ) -> (TriggerState, Ask) {
        let difficulty = (self.progression.level + bias).min(5);
        let ask = |source| Ask {
            difficulty,
            source,
            waited: 0.0,
            deadline: deadline_s,
            counts: true,
            job: None,
        };
        let label = if self.battle.is_some() {
            BATTLE_LABEL
        } else {
            "flask"
        };
        if let Some(task) = self.prefetch.buffer.take(difficulty, topic.as_deref()) {
            return (TriggerState::Active(task), ask(Source::Buffer));
        }
        if self.client.has_api_key() {
            let rx = self.client.request_task_async(difficulty, topic);
            let job = self.activity.start(RequestKind::Question, label);
            let live = Ask {
                job: Some(job),
                ..ask(Source::Live)
            };
            (TriggerState::Loading(rx), live)
        } else {
            let task = task_trigger::pick_fallback_for(&self.fallback_tasks, topic.as_deref());
            let job = self
                .activity
                .instant(RequestKind::Question, label, JobState::Fallback);
            let offline = Ask {
                counts: false,
                job: Some(job),
                ..ask(Source::Fallback)
            };
            (TriggerState::Active(task), offline)
        }
    }

    /// Puts a question on screen. One that's already here counts as ready on
    /// arrival; one still loading is measured until it lands.
    fn show_question(&mut self, (state, ask): (TriggerState, Ask)) {
        match state {
            TriggerState::Loading(_) => self.asking = Some(ask),
            _ => {
                self.asking = None;
                if ask.counts {
                    self.waits.record(ask.source, ask.waited);
                }
            }
        }
        self.trigger_state = state;
    }

    /// Runs every frame, paused or not (`dt` is 0 while paused): collects the
    /// look-ahead answers, polls the shown question and the battle's next one,
    /// and applies the soft deadline.
    fn poll_questions(&mut self, dt: f32) {
        self.prefetch.poll(&mut self.activity);
        if let Some((state, ask)) = &mut self.next_question {
            // Back before it was needed: the player never sees a wait.
            if let Some(arrival) = state.poll(&self.fallback_tasks) {
                ask.source = match arrival {
                    Arrival::Live => Source::Ready,
                    Arrival::Fallback => Source::Fallback,
                };
                if let Some(job) = ask.job {
                    self.activity.finish(job, job_state(arrival));
                }
            }
        }
        if !matches!(self.trigger_state, TriggerState::Loading(_)) {
            return;
        }
        let Some(ask) = &mut self.asking else {
            self.trigger_state.poll(&self.fallback_tasks);
            return;
        };
        ask.waited += dt;
        let landed = self.trigger_state.poll(&self.fallback_tasks);
        let ask = *ask;
        match landed {
            Some(arrival) => {
                self.asking = None;
                if let Some(job) = ask.job {
                    self.activity.finish(job, job_state(arrival));
                }
                let source = match arrival {
                    Arrival::Live => Source::Live,
                    Arrival::Fallback => Source::Fallback,
                };
                self.waits.record(source, ask.waited);
            }
            None if ask.waited >= ask.deadline => {
                // Too slow: show a bank question now and keep the late answer
                // for next time. The hard 8 s timeout stays as the backstop.
                let bank = TriggerState::Active(task_trigger::pick_fallback(&self.fallback_tasks));
                if let TriggerState::Loading(rx) = std::mem::replace(&mut self.trigger_state, bank)
                {
                    self.prefetch.adopt(ask.difficulty, rx);
                }
                self.asking = None;
                if let Some(job) = ask.job {
                    self.activity.finish(job, JobState::Fallback);
                }
                self.waits.record(Source::Deadline, ask.waited);
            }
            None => {}
        }
    }

    /// Starts look-ahead requests for what's coming up the road (FR32).
    /// Live questions win: nothing starts while one is loading, or while the
    /// battle's next one is. Not while paused, and not in a battle.
    fn tick_prefetch(&mut self) {
        if !self.prefetch.enabled() || !self.client.has_api_key() || self.is_paused() {
            return;
        }
        let loading = |s: &TriggerState| matches!(s, TriggerState::Loading(_));
        if loading(&self.trigger_state)
            || self.next_question.as_ref().is_some_and(|(s, _)| loading(s))
        {
            return;
        }
        let range = prefetch::LOOKAHEAD_SCREENS * screen_height();
        let upcoming = self.world.upcoming(self.player.pos.y, range);
        let wants = prefetch::demand(&upcoming, self.progression.level);
        self.prefetch.tick(
            &self.client,
            &wants,
            std::time::Instant::now(),
            &mut self.activity,
        );
    }

    /// Keeps the studio's stock of unseen generated monsters topped up for the
    /// tier the player is heading into, and tells it when a question outranks
    /// it. It's the lowest lane (FR32): it waits while a question is loading,
    /// a look-ahead is in flight, or the game is paused.
    fn tick_studio(&mut self, dt: f32) {
        self.set_studio_gate();
        let Some(studio) = &self.studio else {
            return;
        };

        // One job at a time keeps the queue short and the stock honest, and a
        // new file gets a moment to reach the roster or the buffer (the
        // watcher scans every `watcher::POLL`) before it's counted again. A
        // turned-down or failed job waits longer, so a model that keeps
        // answering badly doesn't burn the token budget in a loop.
        let status = studio.status();
        if status.made != self.studio_made_seen {
            self.studio_made_seen = status.made;
            self.studio_settle = crate::content::watcher::POLL.as_secs_f32() * 3.0;
        }
        let bad = status.rejected + status.failed;
        if bad != self.studio_bad_seen {
            self.studio_bad_seen = bad;
            self.studio_settle = STUDIO_BACKOFF_S;
        }
        self.studio_settle = (self.studio_settle - dt).max(0.0);
        if status.queued + usize::from(status.working) > 0 || self.studio_settle > 0.0 {
            return;
        }

        // 1. Practice, first: a problem the player missed comes back as similar
        //    ones, and a thin buffer at their level gets a pack on the topic
        //    they're weakest at (FR22).
        let level = self.progression.level.clamp(1, 5);
        // Look-ahead requests already on their way count: they'll be ready too.
        let ready = self.prefetch.buffer.counts()[level as usize - 1] + self.prefetch.in_flight();
        let weakest = self.progression.weakest_topic().map(str::to_string);
        let plan = prefetch::plan_lesson(
            level,
            ready,
            weakest.as_deref(),
            self.progression.peek_missed(),
            self.lesson_jobs,
        );
        if let Some(job) = plan {
            if job.like.is_some() {
                self.progression.take_missed();
            }
            self.lesson_jobs += 1;
            studio.enqueue(studio::Job::Lessons(job));
            return;
        }
        // 2. Professor Pebble's tips on a topic that keeps going wrong, once each.
        if let Some(topic) = weakest {
            if self.tips_asked.insert(topic.clone()) {
                studio.enqueue(studio::Job::Tips(TipsJob {
                    npc: "Professor Pebble".into(),
                    topic,
                }));
                return;
            }
        }
        // 3. New monsters for the tier ahead (they join the next run, FR21).
        let tier = self.studio_tier();
        if self.unseen_generated(tier) < STUDIO_STOCK {
            self.enqueue_monster(tier);
        }
    }

    /// The monster tier the player is heading into, which the studio designs for.
    fn studio_tier(&self) -> u32 {
        crate::content::monster_spec::tier_for_meters(self.distance_m() + 100)
    }

    /// Generated monsters of `tier` the player hasn't met yet: the studio's
    /// stock, which it keeps at `STUDIO_STOCK`.
    fn unseen_generated(&self, tier: u32) -> usize {
        self.roster
            .iter()
            .filter(|s| {
                studio::is_generated(&s.id)
                    && s.tier == tier
                    && !self.seen_generated.contains(&s.id)
            })
            .count()
    }

    /// Keeps the creation bar (FR33) up to date with the studio: a circle for
    /// each job it starts, closed (and named, for a monster) when it ends, or
    /// taken off quietly when the model's answer was turned down. Also tells
    /// the bar what is ready ahead of need. Runs every frame; a read-only
    /// mirror, so nothing here touches XP, health or timing (invariant 7).
    fn sync_creation_bar(&mut self) {
        if let Some(studio) = &self.studio {
            for event in studio.drain_events() {
                match event {
                    StudioEvent::Started { seq, kind, label } => {
                        let id = self.activity.start(kind, &label);
                        self.studio_jobs.push((seq, id));
                    }
                    StudioEvent::Finished { seq, made } => {
                        let Some(at) = self.studio_jobs.iter().position(|(s, _)| *s == seq) else {
                            continue;
                        };
                        let (_, id) = self.studio_jobs.swap_remove(at);
                        match made {
                            Some(name) => {
                                self.activity.rename(id, &name);
                                self.activity.finish(id, JobState::Ready);
                            }
                            None => self.activity.cancel(id),
                        }
                    }
                }
            }
        }
        let monsters = self.unseen_generated(self.studio_tier());
        self.activity.set_stock(Stock {
            questions: self.prefetch.buffer.len(),
            monsters,
        });
    }

    /// Tells the studio whether something that outranks it is using the model
    /// (FR32's lanes): the game is paused, a question is loading (the shown one
    /// or a battle's next), or a look-ahead is in flight. It starts no request
    /// while this is set. Runs every frame, paused or not.
    fn set_studio_gate(&self) {
        let Some(studio) = &self.studio else {
            return;
        };
        let loading = |s: &TriggerState| matches!(s, TriggerState::Loading(_));
        let busy = self.is_paused()
            || loading(&self.trigger_state)
            || self.next_question.as_ref().is_some_and(|(s, _)| loading(s))
            || self.prefetch.in_flight() > 0;
        studio.set_live_busy(busy);
    }

    /// Asks the studio for a monster of `tier`, on a maths topic that fits
    /// it, avoiding every name already taken.
    fn enqueue_monster(&self, tier: u32) {
        let Some(studio) = &self.studio else {
            return;
        };
        use crate::content::prompts::TOPICS;
        let made = studio.status().made as usize;
        let band = (tier.clamp(1, 4) as usize - 1) * 2;
        let topic = TOPICS[(band + made % 2) % TOPICS.len()];
        let mut avoid: Vec<String> = MonsterKind::ALL
            .iter()
            .map(|k| k.name().to_string())
            .collect();
        avoid.extend(self.roster.iter().map(|s| s.name.clone()));
        studio.enqueue(studio::Job::Monster(MonsterJob {
            tier,
            topic: topic.to_string(),
            avoid,
        }));
    }

    /// Lets go of the question in flight and the battle's next one, keeping
    /// whatever answers arrive for the buffer instead of throwing the model's
    /// work away. For a new battle, the end of one, and a new run.
    fn stash_questions(&mut self) {
        if let TriggerState::Loading(rx) =
            std::mem::replace(&mut self.trigger_state, TriggerState::Idle)
        {
            let difficulty = self.asking.map_or(1, |a| a.difficulty);
            self.prefetch.adopt(difficulty, rx);
        }
        if let Some(job) = self.asking.and_then(|a| a.job) {
            self.activity.cancel(job);
        }
        self.asking = None;
        if let Some((state, ask)) = self.next_question.take() {
            if let (TriggerState::Loading(_), Some(job)) = (&state, ask.job) {
                self.activity.cancel(job);
            }
            match state {
                TriggerState::Loading(rx) => self.prefetch.adopt(ask.difficulty, rx),
                TriggerState::Active(task) if ask.source != Source::Fallback => {
                    self.prefetch.buffer.push(ask.difficulty, task)
                }
                _ => {}
            }
        }
    }

    pub fn submit_answer(&mut self) {
        let TriggerState::Active(task) = &self.trigger_state else {
            return;
        };

        let given: f64 = match self.input_buffer.trim().parse() {
            Ok(v) => v,
            Err(_) => return,
        };

        let correct = (given - task.answer).abs() < 1e-6;
        let (task_for_record, steps) = (task.clone(), task.explanation.clone());

        if self.player.stunned() {
            // FR12: too dizzy to answer until the stars clear.
            self.toast = Some(("Too dizzy! Wait for the stars to clear...".into(), 1.5));
            return;
        }
        // Count it toward its topic, and show how a wrong one works out (FR22).
        // A dizzy player's refused submit (above) isn't an answer, so it's not here.
        self.progression.record_topic(&task_for_record, correct);
        if !correct && !steps.is_empty() {
            let left = EXPLAIN_BASE_S + EXPLAIN_PER_STEP_S * steps.len() as f32;
            self.explain = Some(Explain {
                problem: task_for_record.problem.clone(),
                steps,
                left,
            });
        }
        if self.battle.is_some() {
            self.battle_answer(correct);
            return;
        }

        self.villain.cue(if correct {
            VillainCue::FlaskRight
        } else {
            VillainCue::FlaskWrong
        });
        if correct {
            self.run.flasks_solved += 1;
            let level = self.progression.level;
            let xp_before = self.progression.lifetime_xp;
            for upgrade in self.progression.record_correct(20) {
                self.toast = Some((format!("Unlocked: {}", upgrade.name), 3.0));
            }
            // A right answer mends half a heart; a level-up refills.
            if self.progression.level > level {
                self.health.refill();
            } else {
                self.health.heal(1);
            }
            let gained = self.progression.lifetime_xp - xp_before;
            self.player.cheer();
            self.item_get = Some((0.0, format!("+{gained} XP!")));
            // A right answer also eases the fever (FR1).
            self.curse.cleanse(curse::FLASK_CLEANSE);
        } else {
            self.progression.record_incorrect();
            // Provisional (FR13 open question): a wrong flask answer
            // costs a heart, so HP has a stake before monsters exist.
            self.damage_player(2, None);
        }

        if let Some(i) = self.active_spot.take() {
            self.world.spots[i].state = if correct {
                SpotState::Solved
            } else {
                SpotState::Missed
            };
        }

        self.trigger_state = TriggerState::Resolved {
            correct,
            cooldown: 1.5,
        };
        self.input_buffer.clear();
    }

    pub fn draw(&self) {
        let rf = self.settings.reduce_flashing;
        if let Some(b) = &self.battle {
            if b.scene_visible() {
                b.draw_scene(&self.player, self.time, rf);
                self.draw_hearts();
                b.draw_transition(vec2(screen_width(), screen_height()) * 0.5, rf);
                return;
            }
        }
        let t = self.time;
        let shake = if self.shake > 0.0 {
            (self.time * 70.0).sin() * 4.0 * (self.shake / 0.25)
        } else {
            0.0
        };
        let view = View::new(self.cam_y + shake);

        self.world.draw_ground(&view, t);
        self.world.draw_manor(&view, t, rf);
        self.world.draw_road(&view, t);
        self.world.draw_lights(&view, t);

        // One back-to-front pass over everything that stands up off the
        // ground, keyed by where it touches the ground, so the player walks
        // behind a fence post or tree north of them and in front of one
        // south of them. Arrows go in by the ground under them, after the
        // player so one just loosed draws over the archer (the sort is
        // stable).
        enum Layer<'a> {
            World(world::Tall<'a>),
            Player,
            Arrow(&'a bow::Arrow),
            Villain,
        }
        let mut items: Vec<(f32, Layer<'_>)> = self
            .world
            .tall_items(&view)
            .into_iter()
            .map(|item| (item.sort_y(), Layer::World(item)))
            .collect();
        items.push((self.player.pos.y, Layer::Player));
        // The doctor (FR23) stands at the top of the road, so they're always
        // north of the player. They stay out of dialogues (a battle never
        // reaches this view) and are only there past the gate.
        let doctor = self.villain.visible() && self.dialogue.is_none();
        if doctor {
            items.push((self.villain.ground_y(), Layer::Villain));
        }
        items.extend(
            self.player
                .arrows
                .iter()
                .map(|a| (a.ground.y, Layer::Arrow(a))),
        );
        items.sort_by(|a, b| a.0.total_cmp(&b.0));

        for (_, item) in &items {
            match item {
                Layer::World(item) => item.draw(&view, t),
                Layer::Player => self.player.draw(
                    &view,
                    world::ground_lift(self.player.pos.y),
                    self.settings.reduce_flashing,
                ),
                Layer::Arrow(a) => a.draw(&view),
                Layer::Villain => {
                    self.villain
                        .draw(&view, world::ground_lift(self.villain.ground_y()), t, rf)
                }
            }
        }

        // The night (FR24): mist over everything, then the screen darkens
        // away from the player, then bats and fireflies on top.
        self.world.draw_mist(&view, t);
        let lifted =
            view.to_screen(self.player.pos) - vec2(0.0, world::ground_lift(self.player.pos.y));
        curse::draw_wisps(lifted, self.curse.level, t);
        // The fever (FR1) closes the dark in around the player, and tints
        // the edges green.
        let fever = ((self.curse.level - curse::SLOW_FROM) / (1.0 - curse::SLOW_FROM)).max(0.0);
        let foot = view.to_screen(self.player.pos) - vec2(0.0, 30.0);
        gfx::vignette(
            foot,
            NIGHT_CLEAR_R * (1.0 - 0.4 * fever),
            NIGHT_DARK_R * (1.0 - 0.3 * fever),
            NIGHT_STRENGTH + 0.2 * fever,
        );
        curse::draw_tint(foot, self.curse.level, t, rf);
        self.world.draw_overhead(&view, t, rf);

        // Last, so the prompt is never hidden behind a tree or a bat.
        if let Some(i) = self.interact_target {
            let base = view.to_screen(self.world.spots[i].pos);
            gfx::key_prompt(base - vec2(0.0, world::ROAD_LIFT + 54.0), "E", t);
        }
        if let Some(z) = self.blocker_target.and_then(|z| self.world.zones.get(z)) {
            let tall = if z.kind.info().boss { 120.0 } else { 74.0 };
            let base = view.to_screen(z.pos);
            gfx::key_prompt(base - vec2(0.0, world::ROAD_LIFT + tall), "E", t);
        }
        if let Some(s) = self
            .world
            .stop_in_reach(self.player.pos)
            .filter(|_| self.stop_target)
        {
            let base = view.to_screen(s.house());
            gfx::key_prompt(base - vec2(0.0, world::ROAD_LIFT + 70.0), "E", t);
        }
        for &(at, age) in &self.sparks {
            let c = view.to_screen(at) - vec2(0.0, world::ROAD_LIFT + 13.0);
            gfx::sparkle_burst(c, age / 0.9);
        }
        if self.npc_in_reach && self.dialogue.is_none() {
            let base = view.to_screen(world::pebble_pos());
            gfx::key_prompt(base - vec2(0.0, 58.0), "E", t);
        }

        // The doctor's speech bubble, above the night so it's always readable.
        if doctor {
            self.villain
                .draw_bubble(&view, world::ground_lift(self.villain.ground_y()));
        }

        if let Some((it, text)) = &self.item_get {
            let foot =
                view.to_screen(self.player.pos) - vec2(0.0, world::ground_lift(self.player.pos.y));
            gfx::item_get(
                foot - vec2(0.0, 62.0),
                *it,
                text,
                self.settings.reduce_flashing,
            );
        }

        self.draw_hearts();
        // Bottom centre, where the dialogue box also sits.
        if self.dialogue.is_none() && self.game_over.is_none() {
            skills::draw_bar(
                &self.player.skills,
                matches!(self.player.action, player::Action::Roll { .. }),
                t,
            );
        }
        if let Some(b) = &self.battle {
            let at = view.to_screen(self.player.pos) - vec2(0.0, 24.0);
            b.draw_transition(at, rf);
        }
    }

    /// HP is always on screen, whatever the HUD setting, and so is the
    /// curse meter under it once the curse has started (FR1).
    fn draw_hearts(&self) {
        for i in 0..self.health.hearts() {
            let fill = self.health.current.saturating_sub(i * 2).min(2);
            gfx::heart(vec2(28.0 + i as f32 * 30.0, 28.0), 24.0, fill);
        }
        let mut y = 50.0;
        if self.health.regenerating() {
            health::draw_regen_bar(vec2(16.0, y), &self.health, self.curse.level >= 1.0);
            y += 16.0;
        }
        if self.curse.started {
            curse::draw_meter(vec2(16.0, y), self.curse.level);
        }
    }

    /// Dimmed "Paused" overlay while paused; a plain dim behind the Game
    /// Over panel.
    pub fn draw_pause_overlay(&self) {
        if self.can_restart() {
            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                screen_height(),
                Color::new(0.0, 0.0, 0.0, 0.35),
            );
            return;
        }
        if !self.is_paused() {
            return;
        }
        let (w, h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        if self.settings_open || self.map_open || self.shop_open {
            // The modal says what's going on; the dim alone is enough.
            return;
        }
        for (text, size, dy) in [("Paused", 64.0, -10.0), ("P to resume", 28.0, 34.0)] {
            let dim = measure_text(text, None, size as u16, 1.0);
            draw_text(text, (w - dim.width) / 2.0, h / 2.0 + dy, size, WHITE);
        }
    }
}
