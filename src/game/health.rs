//! Player HP (FR13), counted in half-hearts so damage can take half a
//! heart. Kept apart from `Progression`: XP and HP are separate concerns.

use macroquad::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Health {
    /// Half-hearts left.
    pub current: u32,
    /// Half-hearts when full (2 per heart).
    pub max: u32,
    /// Seconds since the last hit, for the regen delay.
    since_hurt: f32,
    /// Seconds banked toward the next regenerated half-heart.
    regen: f32,
}

/// Starting hearts.
pub const START_HEARTS: u32 = 5;
/// Regeneration waits this long after a hit before it starts.
pub const REGEN_DELAY_S: f32 = 4.0;
/// Then it restores half a heart this often (a full heart every 16 s).
pub const REGEN_EVERY_S: f32 = 8.0;
/// At a safe stop (FR28) regeneration runs this many times faster: the delay
/// and the wait both shrink, so a half-heart comes back every 2 s.
pub const REST_REGEN_RATE: f32 = 4.0;
/// The most hearts the shop can grow the player to (FR17).
pub const MAX_HEARTS: u32 = 8;

impl Health {
    pub fn new(hearts: u32) -> Self {
        Self {
            current: hearts * 2,
            max: hearts * 2,
            // Starts past the delay, so a first hit's wait is the only one.
            since_hurt: REGEN_DELAY_S,
            regen: 0.0,
        }
    }

    /// Takes `halves` half-hearts, never going below 0. Returns what was
    /// actually lost.
    pub fn damage(&mut self, halves: u32) -> u32 {
        let lost = halves.min(self.current);
        self.current -= lost;
        // A hit pauses regeneration and empties the bar.
        self.since_hurt = 0.0;
        self.regen = 0.0;
        lost
    }

    /// Regenerates over time: half a heart every [`REGEN_EVERY_S`], once
    /// [`REGEN_DELAY_S`] has passed since the last hit. `allowed` is false
    /// while something (the full curse) is holding it off; the bar stops but
    /// keeps what it has. Heals through [`Health::heal`] and returns true on
    /// the frame a half-heart comes back.
    pub fn tick(&mut self, dt: f32, allowed: bool) -> bool {
        self.tick_rate(dt, allowed, 1.0)
    }

    /// [`Health::tick`] with time running `rate` times faster, for resting at a
    /// safe stop (FR28, [`REST_REGEN_RATE`]).
    pub fn tick_rate(&mut self, dt: f32, allowed: bool, rate: f32) -> bool {
        let dt = dt * rate;
        if self.is_dead() || self.current >= self.max {
            self.regen = 0.0;
            return false;
        }
        self.since_hurt += dt;
        if !allowed || self.since_hurt < REGEN_DELAY_S {
            return false;
        }
        self.regen += dt;
        if self.regen < REGEN_EVERY_S {
            return false;
        }
        self.regen -= REGEN_EVERY_S;
        self.heal(1);
        true
    }

    /// 0..1 toward the next regenerated half-heart, for the HUD bar.
    pub fn regen_progress(&self) -> f32 {
        (self.regen / REGEN_EVERY_S).clamp(0.0, 1.0)
    }

    /// True when the bar has something to show: hurt, and still alive.
    pub fn regenerating(&self) -> bool {
        !self.is_dead() && self.current < self.max
    }

    /// Restores `halves` half-hearts, never going above `max`.
    pub fn heal(&mut self, halves: u32) {
        self.current = (self.current + halves).min(self.max);
    }

    /// Adds one heart to the maximum and fills it, up to [`MAX_HEARTS`]
    /// (FR17's shop). Returns false when already at the cap.
    pub fn add_heart(&mut self) -> bool {
        if self.hearts() >= MAX_HEARTS {
            return false;
        }
        self.max += 2;
        self.heal(2);
        true
    }

    pub fn refill(&mut self) {
        self.current = self.max;
        self.regen = 0.0;
    }

    pub fn is_dead(&self) -> bool {
        self.current == 0
    }

    /// Hearts to draw (rounded up).
    pub fn hearts(&self) -> u32 {
        self.max.div_ceil(2)
    }
}

impl Default for Health {
    fn default() -> Self {
        Self::new(START_HEARTS)
    }
}

/// The regen bar under the hearts: fills toward the next half-heart. Same
/// shape as the curse meter beneath it. Dim while the delay after a hit (or
/// the full curse) is holding it.
pub fn draw_regen_bar(top_left: Vec2, h: &Health, held_by_curse: bool) {
    use super::gfx;
    let (w, bh) = (148.0, 10.0);
    draw_text("Heal", top_left.x, top_left.y + bh, 18.0, gfx::MOON);
    let x = top_left.x + 50.0;
    draw_rectangle(x - 1.5, top_left.y - 1.5, w + 3.0, bh + 3.0, gfx::INK);
    draw_rectangle(x, top_left.y, w, bh, Color::new(0.10, 0.06, 0.08, 0.9));
    let waiting = held_by_curse || h.since_hurt < REGEN_DELAY_S;
    let fill = if waiting {
        Color::new(0.62, 0.30, 0.36, 1.0)
    } else {
        Color::new(1.0, 0.42, 0.50, 1.0)
    };
    draw_rectangle(x, top_left.y, w * h.regen_progress(), bh, fill);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_full() {
        let h = Health::new(5);
        assert_eq!((h.current, h.max, h.hearts()), (10, 10, 5));
        assert!(!h.is_dead());
    }

    #[test]
    fn damage_clamps_at_zero() {
        let mut h = Health::new(1);
        assert_eq!(h.damage(1), 1);
        assert_eq!(h.damage(5), 1);
        assert_eq!(h.current, 0);
        assert!(h.is_dead());
        assert_eq!(h.damage(1), 0);
    }

    #[test]
    fn heal_clamps_at_max() {
        let mut h = Health::new(3);
        h.damage(3);
        h.heal(1);
        assert_eq!(h.current, 4);
        h.heal(100);
        assert_eq!(h.current, 6);
    }

    #[test]
    fn refill_restores_max() {
        let mut h = Health::new(2);
        h.damage(3);
        h.refill();
        assert_eq!(h.current, h.max);
    }

    const DT: f32 = 0.1;

    fn run(h: &mut Health, secs: f32, allowed: bool) -> u32 {
        (0..(secs / DT).round() as u32)
            .filter(|_| h.tick(DT, allowed))
            .count() as u32
    }

    #[test]
    fn regen_heals_half_a_heart_each_interval() {
        let mut h = Health::new(5);
        h.damage(4);
        assert_eq!(
            run(&mut h, REGEN_DELAY_S + REGEN_EVERY_S * 2.0 + 0.5, true),
            2
        );
        assert_eq!(h.current, 8);
    }

    #[test]
    fn resting_mends_several_times_faster() {
        let mut slow = Health::new(5);
        slow.damage(8);
        let mut rested = slow;
        let secs = REGEN_DELAY_S + REGEN_EVERY_S * 2.0;
        let a = (0..(secs / DT) as u32)
            .filter(|_| slow.tick(DT, true))
            .count();
        let b = (0..(secs / DT) as u32)
            .filter(|_| rested.tick_rate(DT, true, REST_REGEN_RATE))
            .count();
        assert_eq!(a, 1);
        assert!(b >= 7, "rested mended {b} half-hearts in {secs} s");
        // It still stops when the fever holds it off.
        let mut held = Health::new(5);
        held.damage(4);
        assert!(!(0..300).any(|_| held.tick_rate(DT, false, REST_REGEN_RATE)));
    }

    #[test]
    fn a_heart_can_be_added_up_to_the_cap() {
        let mut h = Health::new(START_HEARTS);
        h.damage(3);
        assert!(h.add_heart());
        assert_eq!((h.max, h.hearts()), (12, 6));
        assert_eq!(h.current, 9);
        while h.add_heart() {}
        assert_eq!(h.hearts(), MAX_HEARTS);
        assert!(!h.add_heart());
    }

    #[test]
    fn regen_waits_after_a_hit_and_restarts() {
        let mut h = Health::new(5);
        h.damage(2);
        run(&mut h, REGEN_DELAY_S + REGEN_EVERY_S * 0.5, true);
        assert!(h.regen_progress() > 0.4);
        h.damage(1);
        assert_eq!(h.regen_progress(), 0.0);
        assert_eq!(run(&mut h, REGEN_DELAY_S * 0.9, true), 0);
        assert_eq!(h.regen_progress(), 0.0);
    }

    #[test]
    fn regen_holds_when_not_allowed_and_keeps_its_progress() {
        let mut h = Health::new(5);
        h.damage(2);
        run(&mut h, REGEN_DELAY_S + REGEN_EVERY_S * 0.5, true);
        let p = h.regen_progress();
        assert_eq!(run(&mut h, 30.0, false), 0);
        assert_eq!(h.regen_progress(), p);
    }

    #[test]
    fn regen_stops_at_full_and_when_dead() {
        let mut h = Health::new(1);
        h.damage(1);
        run(&mut h, 100.0, true);
        assert_eq!(h.current, 2);
        assert_eq!(h.regen_progress(), 0.0);
        assert!(!h.regenerating());
        h.damage(2);
        assert!(h.is_dead());
        assert_eq!(run(&mut h, 100.0, true), 0);
        assert_eq!(h.current, 0);
    }
}
