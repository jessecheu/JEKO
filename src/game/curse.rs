//! The doctor's curse (FR1): a fever that sets in once the player steps onto
//! the road and worsens while they linger. It's what keeps the player moving.
//!
//! Pure state and rules, no drawing input: `Game` decides when to tick it
//! (never while a question is loading or open, in a battle or a dialogue, or
//! paused), and turns a [`Tick::Drain`] into a hit through `damage_player`.
//!
//! - Nothing happens in the hub until the player first passes the gate.
//! - On the road it rises steadily, and each metre of new ground (north of
//!   the furthest point this run) pushes it back, so walking holds it level.
//! - Back in the hub it slowly lifts, and so it does inside a safe stop (FR28).
//! - A correct flask cleanses some of it.
//! - Above [`SLOW_FROM`] it slows the player; at full it drains health.

use super::world;
use macroquad::prelude::*;

/// Seconds to go from clear to full standing still on the road, near the gate.
pub const FULL_AFTER_S: f32 = 45.0;
/// The rise rate grows with distance, up to this many times the base rate
/// at [`RAMP_M`] metres and beyond.
pub const RAMP_MAX: f32 = 1.5;
pub const RAMP_M: f32 = 2000.0;
/// How much each metre of new ground lifts. Walking (≈5.8 m/s) lifts about
/// 0.035/s, more than the curse rises even at the far end of the ramp
/// (≈0.033/s), so a player who keeps walking stays level.
pub const RELIEF_PER_M: f32 = 0.006;
/// How much a correct flask cleanses.
pub const FLASK_CLEANSE: f32 = 0.25;
/// Per second in the hub: full to clear in 20 s.
pub const HUB_LIFT_PER_S: f32 = 0.05;
/// The fever slows the player from this level, down to [`SLOWEST`] at full.
pub const SLOW_FROM: f32 = 0.5;
pub const SLOWEST: f32 = 0.85;
/// At full, a half-heart is lost this often.
pub const DRAIN_EVERY_S: f32 = 3.0;

/// The named stages, for the toasts and the HUD colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Clear,
    Cursed,
    Fever,
    Burning,
}

/// What a tick asks the game to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tick {
    Nothing,
    /// The player just stepped onto the road for the first time this run.
    Started,
    /// The curse got worse and crossed into this stage.
    Worsened(Stage),
    /// Full curse: take half a heart now.
    Drain,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Curse {
    /// 0.0 clear to 1.0 full.
    pub level: f32,
    /// The player has stepped onto the road this run.
    pub started: bool,
    /// The northernmost y reached this run (smaller is further).
    furthest_y: f32,
    /// Seconds spent at full since the last drain.
    drain_t: f32,
    /// The worst stage announced so far, so each toast plays once per climb.
    announced: Stage,
}

impl Default for Curse {
    fn default() -> Self {
        Self::new()
    }
}

impl Curse {
    pub fn new() -> Self {
        Self {
            level: 0.0,
            started: false,
            furthest_y: world::ROAD_SOUTH_END,
            drain_t: 0.0,
            announced: Stage::Clear,
        }
    }

    pub fn stage(&self) -> Stage {
        stage_of(self.level)
    }

    /// Advances the curse by `dt` with the player at world `y`.
    pub fn tick(&mut self, dt: f32, y: f32) -> Tick {
        let in_hub = world::in_hub(y);
        // A safe stop calms it the way the hub does (FR28).
        let calm = in_hub || world::in_safe_stop(y);
        if !self.started {
            if in_hub {
                return Tick::Nothing;
            }
            self.started = true;
            self.furthest_y = y;
            return Tick::Started;
        }

        if calm {
            self.level = (self.level - HUB_LIFT_PER_S * dt).max(0.0);
        } else {
            let metres = ((world::ROAD_START_Y - y) / world::PX_PER_METER).max(0.0);
            let ramp = 1.0 + (RAMP_MAX - 1.0) * (metres / RAMP_M).min(1.0);
            let mut level = self.level + ramp / FULL_AFTER_S * dt;
            if y < self.furthest_y {
                level -= (self.furthest_y - y) / world::PX_PER_METER * RELIEF_PER_M;
                self.furthest_y = y;
            }
            self.level = level.clamp(0.0, 1.0);
        }

        // Once it has lifted back to clear, the toasts can play again.
        let stage = self.stage();
        if stage == Stage::Clear {
            self.announced = Stage::Clear;
        }

        if self.level >= 1.0 {
            self.drain_t += dt;
            if self.drain_t >= DRAIN_EVERY_S {
                self.drain_t -= DRAIN_EVERY_S;
                return Tick::Drain;
            }
        } else {
            self.drain_t = 0.0;
        }

        if stage > self.announced {
            self.announced = stage;
            return Tick::Worsened(stage);
        }
        Tick::Nothing
    }

    /// A correct flask lifts some of the curse.
    pub fn cleanse(&mut self, amount: f32) {
        self.level = (self.level - amount).max(0.0);
        self.drain_t = 0.0;
    }

    /// Multiplies the player's speed: 1.0 up to [`SLOW_FROM`], easing down to
    /// [`SLOWEST`] at full.
    pub fn speed_factor(&self) -> f32 {
        let t = ((self.level - SLOW_FROM) / (1.0 - SLOW_FROM)).clamp(0.0, 1.0);
        1.0 + (SLOWEST - 1.0) * t
    }

    /// Dev hook: start the curse at `level` (`JEKO_DEV=curse@…`).
    pub fn force(&mut self, level: f32) {
        self.started = true;
        self.level = level.clamp(0.0, 1.0);
    }
}

pub fn stage_of(level: f32) -> Stage {
    if level >= 1.0 {
        Stage::Burning
    } else if level >= SLOW_FROM {
        Stage::Fever
    } else if level >= 0.2 {
        Stage::Cursed
    } else {
        Stage::Clear
    }
}

/// The fever's sickly tint: the doctor's lab green, dulled.
pub const SICK_GREEN: Color = Color::new(0.30, 0.62, 0.20, 1.0);

/// What the curse does to the screen, by level: a green tint closing in
/// from the edges. `pulse` (0..1) beats at full; it's held steady when
/// flashing is reduced.
pub fn draw_tint(focus: Vec2, level: f32, t: f32, reduce_flashing: bool) {
    if level <= 0.0 {
        return;
    }
    let pulse = if level >= 1.0 && !reduce_flashing {
        0.5 + 0.5 * (t * 4.0).sin()
    } else {
        0.0
    };
    let inner = 560.0 - 300.0 * level;
    let outer = inner + 360.0;
    let a = 0.42 * level + 0.12 * pulse;
    let clear = super::gfx::with_alpha(SICK_GREEN, 0.0);
    let sick = super::gfx::with_alpha(SICK_GREEN, a);
    let far = screen_width().max(screen_height()) * 2.0 + outer;
    super::gfx::gradient_ring(focus, Vec2::splat(inner), Vec2::splat(outer), clear, sick);
    super::gfx::gradient_ring(focus, Vec2::splat(outer), Vec2::splat(far), sick, sick);
}

/// Green wisps curling up off the hero, more of them as the curse worsens.
/// `foot` is the hero's feet on screen.
pub fn draw_wisps(foot: Vec2, level: f32, t: f32) {
    let n = (level * 6.0).ceil() as u64;
    for i in 0..n {
        let seed = i * 7 + 3;
        let period = 1.6 + super::gfx::hash01(seed) * 0.8;
        let phase = (t / period + super::gfx::hash01(seed + 1)).fract();
        let x = (super::gfx::hash01(seed + 2) - 0.5) * 26.0 + (t * 2.0 + i as f32).sin() * 4.0;
        let y = -8.0 - phase * 56.0;
        let fade = (1.0 - phase) * phase * 4.0;
        let c = super::gfx::with_alpha(super::gfx::LAB_GREEN, 0.45 * fade * level.max(0.4));
        draw_circle(foot.x + x, foot.y + y, 2.6 + phase * 2.0, c);
    }
}

/// The curse meter under the hearts: a bar labelled "Curse", from pale to
/// burning green.
pub fn draw_meter(top_left: Vec2, level: f32) {
    let (w, h) = (148.0, 10.0);
    let label = "Curse";
    draw_text(label, top_left.x, top_left.y + h, 18.0, super::gfx::MOON);
    let x = top_left.x + 50.0;
    draw_rectangle(x - 1.5, top_left.y - 1.5, w + 3.0, h + 3.0, super::gfx::INK);
    draw_rectangle(x, top_left.y, w, h, Color::new(0.08, 0.10, 0.10, 0.9));
    let fill = super::gfx::mix(SICK_GREEN, super::gfx::LAB_GREEN, level);
    draw_rectangle(x, top_left.y, w * level, h, fill);
    // Where the fever (slowing) starts.
    draw_line(
        x + w * SLOW_FROM,
        top_left.y,
        x + w * SLOW_FROM,
        top_left.y + h,
        1.0,
        super::gfx::INK,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const GATE: f32 = world::ROAD_SOUTH_END;
    const DT: f32 = 1.0 / 60.0;

    fn run(c: &mut Curse, secs: f32, y: f32) -> Vec<Tick> {
        let mut ticks = Vec::new();
        for _ in 0..(secs / DT).round() as usize {
            let t = c.tick(DT, y);
            if t != Tick::Nothing {
                ticks.push(t);
            }
        }
        ticks
    }

    #[test]
    fn nothing_happens_in_the_hub_before_the_road() {
        let mut c = Curse::new();
        assert!(run(&mut c, 120.0, world::PLAYER_START_Y).is_empty());
        assert!(
            run(&mut c, 10.0, GATE + 1.0).is_empty(),
            "just south of the gate is still the hub"
        );
        assert!(!c.started);
        assert_eq!(c.level, 0.0);
    }

    #[test]
    fn starts_the_first_time_the_player_is_past_the_gate() {
        let mut c = Curse::new();
        assert_eq!(c.tick(DT, GATE - 1.0), Tick::Started);
        assert!(c.started);
        assert_eq!(c.tick(DT, GATE - 1.0), Tick::Nothing);
    }

    #[test]
    fn standing_still_on_the_road_fills_it_and_then_drains() {
        let mut c = Curse::new();
        c.tick(DT, GATE - 10.0);
        let ticks = run(&mut c, FULL_AFTER_S + 0.5, GATE - 10.0);
        assert_eq!(c.level, 1.0);
        assert_eq!(
            ticks,
            vec![
                Tick::Worsened(Stage::Cursed),
                Tick::Worsened(Stage::Fever),
                Tick::Worsened(Stage::Burning)
            ]
        );
        let drains = run(&mut c, DRAIN_EVERY_S * 3.0 + 0.1, GATE - 10.0);
        assert_eq!(drains, vec![Tick::Drain; 3]);
    }

    #[test]
    fn walking_north_holds_it_back() {
        let mut c = Curse::new();
        c.force(0.3);
        c.furthest_y = GATE;
        // Walk 60 s at base speed, far up the road where the ramp is highest.
        let mut y = GATE - RAMP_M * world::PX_PER_METER;
        c.furthest_y = y;
        for _ in 0..3600 {
            y -= 140.0 * DT;
            c.tick(DT, y);
        }
        assert!(c.level < 0.3, "level rose to {}", c.level);
    }

    #[test]
    fn going_back_over_old_ground_gives_no_relief() {
        let mut c = Curse::new();
        c.tick(DT, GATE - 2000.0);
        let before = c.level;
        // South again, then back north to where we were: no new ground.
        run(&mut c, 1.0, GATE - 1000.0);
        run(&mut c, 1.0, GATE - 2000.0);
        assert!(c.level > before);
    }

    #[test]
    fn the_hub_slowly_lifts_it() {
        let mut c = Curse::new();
        c.tick(DT, GATE - 10.0);
        c.force(1.0);
        run(&mut c, 10.0, world::PLAYER_START_Y);
        assert!((c.level - 0.5).abs() < 0.02, "level {}", c.level);
        run(&mut c, 15.0, world::PLAYER_START_Y);
        assert_eq!(c.level, 0.0);
        assert!(c.started, "lifting doesn't reset the run");
    }

    #[test]
    fn a_safe_stop_lifts_it_like_the_hub_and_the_road_beside_it_does_not() {
        let (k, stop) = (15..16)
            .filter_map(|k| world::safe_stop_for_chunk(k).map(|s| (k, s)))
            .next()
            .expect("a stop in chunk 15");
        let mut c = Curse::new();
        c.tick(DT, GATE - 10.0);
        c.force(1.0);
        run(&mut c, 10.0, stop.y);
        assert!((c.level - 0.5).abs() < 0.02, "level {}", c.level);
        // Just past the stop's span it climbs again.
        let past = stop.y - world::STOP_HALF_LEN - 20.0;
        assert!(!world::in_safe_stop(past), "chunk {k}");
        // (The first step north is new ground and relieves it; settle that.)
        run(&mut c, 0.1, past);
        c.force(0.5);
        run(&mut c, 5.0, past);
        assert!(c.level > 0.5, "level {}", c.level);
    }

    #[test]
    fn full_curse_cannot_drain_a_player_resting_at_a_stop() {
        let stop = world::safe_stop_for_chunk(15).unwrap();
        let mut c = Curse::new();
        c.tick(DT, GATE - 10.0);
        c.force(1.0);
        assert!(!run(&mut c, 20.0, stop.y).contains(&Tick::Drain) || c.level < 1.0);
        assert!(c.level < 1.0);
    }

    #[test]
    fn toasts_replay_only_after_it_clears() {
        let mut c = Curse::new();
        c.tick(DT, GATE - 10.0);
        c.force(0.6);
        assert_eq!(c.tick(DT, GATE - 10.0), Tick::Worsened(Stage::Fever));
        c.cleanse(0.15);
        c.force(0.6);
        assert_eq!(
            c.tick(DT, GATE - 10.0),
            Tick::Nothing,
            "no repeat while still cursed"
        );
        c.cleanse(1.0);
        c.tick(DT, world::PLAYER_START_Y);
        c.force(0.6);
        assert_eq!(c.tick(DT, GATE - 10.0), Tick::Worsened(Stage::Fever));
    }

    #[test]
    fn cleanse_and_speed_clamp() {
        let mut c = Curse::new();
        c.force(0.2);
        c.cleanse(FLASK_CLEANSE);
        assert_eq!(c.level, 0.0);
        assert_eq!(c.speed_factor(), 1.0);
        c.force(SLOW_FROM);
        assert_eq!(c.speed_factor(), 1.0);
        c.force(1.0);
        assert!((c.speed_factor() - SLOWEST).abs() < 1e-6);
    }
}
