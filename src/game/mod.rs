pub mod battle;
pub mod bow;
pub mod curse;
pub mod decor;
pub mod dialogue;
pub mod flicker;
pub mod gfx;
pub mod health;
pub mod manor;
pub mod map;
pub mod monster;
pub mod monster_rig;
pub mod player;
pub mod skills;
pub mod task_trigger;
pub mod world;

use crate::content::{ContentEvent, Roster};
use crate::llm::cerebras_client::CerebrasClient;
use crate::llm::MathTask;
use crate::progression::Progression;
use battle::{Battle, Phase};
use curse::Curse;
use flicker::FlickerKind;
use health::Health;
use macroquad::prelude::*;
use monster::{MonsterKind, Species};
use player::Player;
use std::sync::mpsc::{Receiver, TryRecvError};
use task_trigger::TriggerState;
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

/// Player-facing switches, edited in the settings modal (FR7) and read by
/// the systems they control. Not saved between runs yet.
#[derive(Clone, Debug)]
pub struct Settings {
    pub show_token_panel: bool,
    pub show_hud: bool,
    /// Swaps flicker effects (spawn, damage) for a steady tint.
    pub reduce_flashing: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_token_panel: true,
            show_hud: true,
            reduce_flashing: false,
        }
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
    /// The discovery map is open (FR27). It freezes the world like the
    /// settings modal, and only opens when nothing else has the screen.
    pub map_open: bool,
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
}

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
            map_open: false,
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
            client,
            fallback_tasks,
            cam_y: camera_target(start.y),
            active_spot: None,
            roster: Roster::default(),
            content_rx: None,
        }
    }

    /// Starts taking content events from a watcher
    /// (`content::watcher::spawn`). Without one the roster stays empty
    /// and only the built-in monsters exist.
    pub fn watch_content(&mut self, rx: Receiver<ContentEvent>) {
        self.content_rx = Some(rx);
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
        for event in events {
            self.roster.apply(&event);
            match &event {
                ContentEvent::Monster { spec, reloaded } => {
                    if let Some(b) = &mut self.battle {
                        b.respec(spec);
                    }
                    if *reloaded {
                        self.toast = Some((format!("Reloaded {}", spec.name), 1.5));
                    }
                }
                ContentEvent::Invalid { file, error } => {
                    eprintln!("content: {file}: {error} (keeping the last good version)");
                    self.toast = Some((format!("{file}: {error}"), 4.0));
                }
                ContentEvent::Removed { .. } => {}
            }
        }
    }

    /// True while the world is frozen: paused with P or the settings
    /// modal is open.
    pub fn is_paused(&self) -> bool {
        self.paused || self.settings_open || self.map_open
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
            if self.map_open {
                if is_key_pressed(MAP_KEY) || is_key_pressed(KeyCode::Escape) {
                    self.map_open = false;
                }
            } else if is_key_pressed(MAP_KEY) && self.can_open_map() {
                self.map_open = true;
            } else if is_key_pressed(SETTINGS_KEY)
                || (self.settings_open && is_key_pressed(KeyCode::Escape))
            {
                self.settings_open = !self.settings_open;
            } else if !self.settings_open && is_key_pressed(PAUSE_KEY) {
                self.paused = !self.paused;
            }
        }
        if self.is_paused() {
            // Keep polling so a Cerebras response that lands mid-pause is
            // stored, not lost; nothing else advances.
            self.trigger_state.poll(&self.fallback_tasks);
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
        if let Some((t, _)) = &mut self.item_get {
            *t += dt;
            if *t > player::CHEER_S + 0.4 {
                self.item_get = None;
            }
        }
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
            self.player.update(
                dt,
                self.progression.speed_multiplier() * self.curse.speed_factor(),
                world::clamp_to_path,
            );
            // The curse holds still while a question is loading or open
            // (owner, FR1), like the player.
            self.tick_curse(dt);
            if self.game_over.is_some() {
                return;
            }
        }
        self.discovery.note(self.player.pos.y);

        let target = camera_target(self.player.pos.y);
        self.cam_y += (target - self.cam_y) * (1.0 - (-dt * CAMERA_FOLLOW).exp());
        self.world.ensure_generated(self.cam_y - world::CHUNK_H);

        self.trigger_state.poll(&self.fallback_tasks);

        // Stepping into a monster's patch of ferns starts a battle.
        if !busy {
            if let Some(z) = self.world.encounter_at(self.player.pos) {
                self.start_battle(Species::Builtin(self.world.zones[z].kind), Some(z));
                return;
            }
        }

        self.interact_target = match self.trigger_state {
            TriggerState::Idle => self.world.interactable_spot(self.player.pos),
            _ => None,
        };
        self.npc_in_reach = matches!(self.trigger_state, TriggerState::Idle)
            && self.interact_target.is_none()
            && self.player.pos.distance(world::pebble_pos()) < world::NPC_TALK_DIST;

        if is_key_pressed(INTERACT_KEY) {
            match (&self.trigger_state, self.interact_target) {
                (TriggerState::Idle, Some(i)) => self.open_spot(i),
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
        if let Some(bank) = self
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

    /// Starts a battle with `species`. `zone` is the encounter zone to
    /// clear on victory (`None` for a scripted dev battle).
    fn start_battle(&mut self, species: Species, zone: Option<usize>) {
        self.trigger_state = TriggerState::Idle;
        self.interact_target = None;
        self.input_buffer.clear();
        self.player.action = player::Action::None;
        // The battle view puts the monster on the right.
        self.player.facing = 1.0;
        self.battle = Some(Battle::new(species, zone.unwrap_or(usize::MAX)));
    }

    /// Advances the battle's phases: transition, intro, attacks, faint.
    /// The player is held still throughout.
    fn update_battle(&mut self, dt: f32) {
        self.player.stop(dt);
        self.trigger_state.poll(&self.fallback_tasks);
        if is_key_pressed(INTERACT_KEY) && matches!(self.trigger_state, TriggerState::Active(_)) {
            self.focus_answer = true;
        }
        let Some(b) = self.battle.as_mut() else {
            return;
        };

        let (mut next_question, mut monster_hit, mut won, mut done) = (false, None, false, false);
        b.phase = match b.phase {
            Phase::Enter(t) if t + dt >= battle::ENTER_S => {
                next_question = true;
                Phase::Intro(0.0)
            }
            Phase::Enter(t) => Phase::Enter(t + dt),
            Phase::Intro(t) if t + dt >= battle::INTRO_S => Phase::Question,
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
            self.trigger_state = self.request_task(info.difficulty_bias);
        }
        if won {
            if self.player.hat_stolen {
                self.player.hat_stolen = false;
            }
            if let Some(z) = self.world.zones.get_mut(zone) {
                z.cleared = true;
            }
            self.run.monsters_beaten += 1;
            self.trigger_state = TriggerState::Idle;
            self.toast = Some((format!("You beat the {name}!"), 2.5));
        }
        if done {
            self.battle = None;
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
        self.health.damage(halves);
        if self.health.is_dead() {
            self.die();
            return true;
        }
        self.player
            .start_flicker(FlickerKind::Damage, HURT_INVULN_S);
        if let Some(src) = from {
            let away = (self.player.pos - src).normalize_or(vec2(0.0, 1.0));
            self.player.pos = world::clamp_to_path(self.player.pos + away * KNOCKBACK_PX);
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
        self.player = Player::new(start);
        self.world = World::new();
        self.progression = Progression::new();
        self.trigger_state = TriggerState::Idle;
        self.input_buffer.clear();
        self.focus_answer = false;
        self.toast = None;
        self.paused = false;
        self.settings_open = false;
        self.map_open = false;
        self.discovery = map::Discovery::new();
        self.health = Health::default();
        self.curse = Curse::new();
        self.run = RunStats::default();
        self.game_over = None;
        self.shake = 0.0;
        self.interact_target = None;
        self.cam_y = camera_target(start.y);
        self.active_spot = None;
        self.battle = None;
        self.item_get = None;
        self.dialogue = None;
        self.npc_in_reach = false;
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
             arrows={} roster={} curse={:.2}",
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
            self.roster.len(),
            self.curse.level,
        )
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
                if let Some(z) = self.world.zones.iter().find(|z| !z.cleared) {
                    self.player.pos = z.pos + vec2(0.0, 10.0);
                    self.cam_y = camera_target(self.player.pos.y);
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
                (TriggerState::Idle, None) if self.npc_in_reach => self.talk_to_pebble(),
                _ => {}
            },
            "stun" => self.player.stun(2.5),
            "cheer" => {
                self.player.cheer();
                self.item_get = Some((0.0, "+20 XP!".into()));
            }
            "pause" => self.paused = true,
            "settings" => self.settings_open = true,
            "map" => self.map_open = true,
            // 120 m up the road, discovered, for a shot of a filled-in map.
            "far" => {
                let y = world::ROAD_START_Y - 120.0 * world::PX_PER_METER;
                self.world.ensure_generated(y - world::CHUNK_H);
                self.player.pos = vec2(world::path_center(y), y);
                self.discovery.note(y);
            }
            "calm" => self.settings.reduce_flashing = true,
            // The curse (FR1) at the fever stage, and at full.
            "curse" => self.curse.force(0.75),
            "cursefull" => self.curse.force(1.0),
            // Hides both side panels, so a screenshot shows the whole world.
            "nohud" => {
                self.settings.show_hud = false;
                self.settings.show_token_panel = false;
            }
            _ => eprintln!("JEKO_DEV: unknown event {name}"),
        }
    }

    fn open_spot(&mut self, i: usize) {
        self.world.spots[i].state = SpotState::Pending;
        self.active_spot = Some(i);
        self.interact_target = None;
        self.trigger_state = self.request_task(0);
    }

    /// A fresh question: live from the model if there's a key, otherwise
    /// from the fallback bank. `bias` raises the difficulty (tough
    /// monsters ask harder questions).
    fn request_task(&self, bias: u32) -> TriggerState {
        let difficulty = (self.progression.level + bias).min(5);
        if self.client.has_api_key() {
            TriggerState::Loading(self.client.request_task_async(difficulty))
        } else {
            TriggerState::Active(task_trigger::pick_fallback(&self.fallback_tasks))
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

        if self.player.stunned() {
            // FR12: too dizzy to answer until the stars clear.
            self.toast = Some(("Too dizzy! Wait for the stars to clear...".into(), 1.5));
            return;
        }
        if self.battle.is_some() {
            self.battle_answer(correct);
            return;
        }

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
        }
        let mut items: Vec<(f32, Layer<'_>)> = self
            .world
            .tall_items(&view)
            .into_iter()
            .map(|item| (item.sort_y(), Layer::World(item)))
            .collect();
        items.push((self.player.pos.y, Layer::Player));
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
        if self.npc_in_reach && self.dialogue.is_none() {
            let base = view.to_screen(world::pebble_pos());
            gfx::key_prompt(base - vec2(0.0, 58.0), "E", t);
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
        if self.curse.started {
            curse::draw_meter(vec2(16.0, 50.0), self.curse.level);
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
        if self.settings_open || self.map_open {
            // The modal says what's going on; the dim alone is enough.
            return;
        }
        for (text, size, dy) in [("Paused", 64.0, -10.0), ("P to resume", 28.0, 34.0)] {
            let dim = measure_text(text, None, size as u16, 1.0);
            draw_text(text, (w - dim.width) / 2.0, h / 2.0 + dy, size, WHITE);
        }
    }
}
