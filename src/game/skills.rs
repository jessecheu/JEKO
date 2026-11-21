//! Movement skills (FR26): the roll's charges, the sprint's stamina, and
//! the skill bar that shows them at the bottom centre of the screen.
//!
//! The rules are plain timers advanced by `dt`, with no input reads, so
//! they freeze with a pause and can be unit-tested. `Player` owns a
//! [`Skills`] and asks it before starting a roll or sprinting.

use super::gfx::{self, INK, LANTERN, MOON};
use macroquad::prelude::*;
use std::f32::consts::{PI, TAU};

/// Rolls the player can chain before the lockout.
pub const ROLL_CHARGES: u32 = 3;
/// A spent charge trickles back after this long without rolling.
pub const ROLL_RECHARGE_S: f32 = 1.0;
/// Spending the last charge locks the roll for this long, then refills all.
pub const ROLL_LOCKOUT_S: f32 = 1.5;
/// Seconds of sprinting a full stamina meter holds.
pub const SPRINT_S: f32 = 3.0;
/// Seconds to refill stamina from empty, once it starts coming back.
pub const STAMINA_REFILL_S: f32 = 4.0;
/// Stamina starts coming back this long after the sprint stops.
pub const STAMINA_REGEN_DELAY_S: f32 = 0.5;
/// Running dry leaves the player winded until stamina is back to this.
pub const WINDED_UNTIL: f32 = 0.35;

/// The roll's charges: chain up to [`ROLL_CHARGES`] rolls, then wait out a
/// short lockout. A charge also comes back on its own after a rest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RollCharges {
    charges: u32,
    /// Seconds since the last roll, towards the next trickled charge.
    recharge: f32,
    /// Seconds of lockout left; nonzero only once every charge is spent.
    lockout: f32,
}

impl Default for RollCharges {
    fn default() -> Self {
        Self {
            charges: ROLL_CHARGES,
            recharge: 0.0,
            lockout: 0.0,
        }
    }
}

impl RollCharges {
    pub fn charges(&self) -> u32 {
        self.charges
    }

    /// Seconds of lockout left, while every charge is spent.
    pub fn lockout_left(&self) -> Option<f32> {
        (self.lockout > 0.0).then_some(self.lockout)
    }

    /// How far the next trickled charge is, 0..1; `None` when full or
    /// locked out.
    pub fn recharge_progress(&self) -> Option<f32> {
        (self.lockout <= 0.0 && self.charges < ROLL_CHARGES)
            .then_some((self.recharge / ROLL_RECHARGE_S).clamp(0.0, 1.0))
    }

    /// Spends a charge if there is one. Spending the last starts the lockout.
    pub fn try_use(&mut self) -> bool {
        if self.charges == 0 {
            return false;
        }
        self.charges -= 1;
        self.recharge = 0.0;
        if self.charges == 0 {
            self.lockout = ROLL_LOCKOUT_S;
        }
        true
    }

    pub fn update(&mut self, dt: f32) {
        if self.lockout > 0.0 {
            self.lockout -= dt;
            if self.lockout <= 0.0 {
                *self = Self::default();
            }
            return;
        }
        if self.charges < ROLL_CHARGES {
            self.recharge += dt;
            if self.recharge >= ROLL_RECHARGE_S {
                self.charges += 1;
                self.recharge = 0.0;
            }
        }
    }
}

/// The sprint's energy: it drains while sprinting and comes back over time.
/// Running it dry leaves the player winded until it recovers to
/// [`WINDED_UNTIL`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stamina {
    /// 0..1.
    energy: f32,
    /// Seconds since the last sprint, for the regen delay.
    rest: f32,
    winded: bool,
    /// Sprinting this frame, for the bar's highlight.
    sprinting: bool,
}

impl Default for Stamina {
    fn default() -> Self {
        Self {
            energy: 1.0,
            rest: STAMINA_REGEN_DELAY_S,
            winded: false,
            sprinting: false,
        }
    }
}

impl Stamina {
    pub fn energy(&self) -> f32 {
        self.energy
    }

    pub fn winded(&self) -> bool {
        self.winded
    }

    pub fn sprinting(&self) -> bool {
        self.sprinting
    }

    pub fn can_sprint(&self) -> bool {
        !self.winded && self.energy > 0.0
    }

    /// Advances one frame. `sprinting` is whether the player sprinted this
    /// frame; it only counts if they [`can_sprint`](Self::can_sprint).
    pub fn update(&mut self, dt: f32, sprinting: bool) {
        self.sprinting = sprinting && self.can_sprint();
        if self.sprinting {
            self.rest = 0.0;
            self.energy -= dt / SPRINT_S;
            if self.energy <= 0.0 {
                self.energy = 0.0;
                self.winded = true;
            }
            return;
        }
        self.rest += dt;
        if self.rest >= STAMINA_REGEN_DELAY_S {
            self.energy = (self.energy + dt / STAMINA_REFILL_S).min(1.0);
        }
        if self.winded && self.energy >= WINDED_UNTIL {
            self.winded = false;
        }
    }

    /// Empties the meter and leaves the player winded (the `spent` dev event).
    pub fn exhaust(&mut self) {
        self.energy = 0.0;
        self.rest = 0.0;
        self.winded = true;
        self.sprinting = false;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Skills {
    pub roll: RollCharges,
    pub stamina: Stamina,
}

impl Skills {
    pub fn update(&mut self, dt: f32, sprinting: bool) {
        self.roll.update(dt);
        self.stamina.update(dt, sprinting);
    }
}

// ---------------------------------------------------------------- drawing

const SLOT: f32 = 52.0;
const SLOT_GAP: f32 = 18.0;
const DEPTH: f32 = 4.0;
const METER_H: f32 = 8.0;
const BOTTOM_MARGIN: f32 = 14.0;
const FACE: Color = Color::new(1.0, 0.97, 0.87, 1.0);
const SIDE: Color = Color::new(0.84, 0.63, 0.35, 1.0);
const WOOD: Color = Color::new(0.29, 0.19, 0.09, 1.0);
const SPENT: Color = Color::new(0.25, 0.12, 0.10, 0.55);
const WINDED: Color = Color::new(0.93, 0.32, 0.26, 1.0);

/// The skill bar, bottom centre, in screen space: Roll (Ctrl) with its
/// charge pips, and Sprint (Shift) with its stamina meter. `rolling` lights
/// the roll slot while a roll plays. Nothing here blinks, so it's safe
/// under reduce flashing as is.
pub fn draw_bar(skills: &Skills, rolling: bool, t: f32) {
    let total_w = SLOT * 2.0 + SLOT_GAP;
    let left = ((screen_width() - total_w) * 0.5).round();
    let label_y = screen_height() - BOTTOM_MARGIN;
    let meter_y = label_y - 18.0 - METER_H;
    let slot_y = meter_y - 6.0 - SLOT - DEPTH;

    let roll_x = left;
    let roll_lock = skills.roll.lockout_left();
    slot(roll_x, slot_y, rolling, roll_lock.is_some());
    roll_icon(
        vec2(roll_x, slot_y) + vec2(SLOT, SLOT) * 0.5,
        roll_lock.is_some(),
    );
    if let Some(left) = roll_lock {
        sweep(
            vec2(roll_x, slot_y) + vec2(SLOT, SLOT) * 0.5,
            SLOT * 0.42,
            left / ROLL_LOCKOUT_S,
        );
        centred_text(
            &format!("{left:.1}"),
            vec2(roll_x + SLOT * 0.5, slot_y + SLOT * 0.5),
            22,
            WHITE,
        );
    }
    roll_pips(&skills.roll, roll_x, meter_y);
    key_label("Ctrl", vec2(roll_x + SLOT * 0.5, label_y));

    let sprint_x = left + SLOT + SLOT_GAP;
    let st = &skills.stamina;
    slot(sprint_x, slot_y, st.sprinting(), st.winded());
    sprint_icon(
        vec2(sprint_x, slot_y) + vec2(SLOT, SLOT) * 0.5,
        st.winded(),
        st.sprinting(),
        t,
    );
    stamina_meter(st, sprint_x, meter_y);
    key_label(
        if st.winded() { "Winded" } else { "Shift" },
        vec2(sprint_x + SLOT * 0.5, label_y),
    );
}

/// A keycap-like slot, matching `gfx::key_prompt`. `active` warms the face;
/// `spent` greys it.
fn slot(x: f32, y: f32, active: bool, spent: bool) {
    let r = 8.0;
    gfx::rounded_rect(
        x - 2.0,
        y - 2.0,
        SLOT + 4.0,
        SLOT + DEPTH + 4.0,
        r + 2.0,
        INK,
    );
    let (face, side) = if spent {
        (gfx::shade(FACE, 0.55), gfx::shade(SIDE, 0.55))
    } else if active {
        (gfx::mix(FACE, LANTERN, 0.35), SIDE)
    } else {
        (FACE, SIDE)
    };
    if active && !spent {
        gfx::glow(
            vec2(x + SLOT * 0.5, y + SLOT * 0.5),
            SLOT * 0.95,
            SLOT * 0.95,
            gfx::with_alpha(LANTERN, 0.35),
        );
    }
    gfx::rounded_rect(x, y, SLOT, SLOT + DEPTH, r, side);
    gfx::rounded_rect(x, y, SLOT, SLOT, r, face);
}

/// A curled arrow: the roll.
fn roll_icon(c: Vec2, spent: bool) {
    let ink = if spent {
        gfx::with_alpha(WOOD, 0.5)
    } else {
        WOOD
    };
    let r = 14.0;
    let start = -PI * 0.35;
    let sweep = PI * 1.55;
    let pts: Vec<Vec2> = (0..=20)
        .map(|i| {
            let a = start + sweep * i as f32 / 20.0;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    gfx::polyline(&pts, 4.5, ink);
    // Arrowhead at the end of the curl, pointing along it.
    let a = start + sweep;
    let tip_dir = vec2(-a.sin(), a.cos());
    let end = c + vec2(a.cos(), a.sin()) * r;
    let out = vec2(a.cos(), a.sin());
    draw_triangle(end + tip_dir * 8.0, end + out * 7.0, end - out * 7.0, ink);
    // The hero, tucked into a ball in the middle.
    draw_circle(c.x, c.y, 5.0, ink);
}

/// Running chevrons with speed lines: the sprint.
fn sprint_icon(c: Vec2, winded: bool, sprinting: bool, t: f32) {
    let ink = if winded {
        gfx::with_alpha(WOOD, 0.5)
    } else {
        WOOD
    };
    // While sprinting the chevrons push forward a little.
    let push = if sprinting {
        ((t * 10.0).sin() * 0.5 + 0.5) * 3.0
    } else {
        0.0
    };
    for (i, dx) in [-4.0, 8.0].into_iter().enumerate() {
        let x = c.x + dx + push * (i as f32 + 1.0) * 0.5;
        gfx::polyline(
            &[
                vec2(x - 7.0, c.y - 11.0),
                vec2(x + 3.0, c.y),
                vec2(x - 7.0, c.y + 11.0),
            ],
            5.0,
            ink,
        );
    }
    for (dy, len) in [(-7.0, 9.0), (0.0, 13.0), (7.0, 9.0)] {
        let x1 = c.x - 12.0 - push;
        gfx::line(vec2(x1 - len, c.y + dy), vec2(x1, c.y + dy), 2.5, ink);
    }
    if winded {
        // A sweat drop: out of breath.
        let d = c + vec2(15.0, -14.0);
        draw_circle(d.x, d.y + 2.0, 4.0, Color::new(0.55, 0.78, 1.0, 1.0));
        draw_triangle(
            d + vec2(-3.6, 0.6),
            d + vec2(3.6, 0.6),
            d + vec2(0.0, -6.0),
            Color::new(0.55, 0.78, 1.0, 1.0),
        );
    }
}

/// The dark, shrinking pie over a locked slot: `frac` of the lockout left.
fn sweep(c: Vec2, r: f32, frac: f32) {
    let color = Color::new(0.05, 0.03, 0.06, 0.6);
    let n = 32;
    let span = TAU * frac.clamp(0.0, 1.0);
    // From twelve o'clock, clockwise, ending where the remaining time ends.
    let start = -PI * 0.5 + (TAU - span);
    for i in 0..n {
        let a0 = start + span * i as f32 / n as f32;
        let a1 = start + span * (i + 1) as f32 / n as f32;
        draw_triangle(
            c,
            c + vec2(a0.cos(), a0.sin()) * r,
            c + vec2(a1.cos(), a1.sin()) * r,
            color,
        );
    }
}

/// One pip per charge under the roll slot. The next charge to come back
/// grows inside its empty pip.
fn roll_pips(roll: &RollCharges, x: f32, y: f32) {
    let r = METER_H * 0.5 + 1.0;
    let step = SLOT / ROLL_CHARGES as f32;
    for i in 0..ROLL_CHARGES {
        let c = vec2(x + step * (i as f32 + 0.5), y + METER_H * 0.5);
        draw_circle(c.x, c.y, r + 1.5, INK);
        draw_circle(c.x, c.y, r, SPENT);
        if i < roll.charges() {
            draw_circle(c.x, c.y, r, LANTERN);
            draw_circle(
                c.x - r * 0.3,
                c.y - r * 0.3,
                r * 0.35,
                gfx::with_alpha(WHITE, 0.6),
            );
        } else if i == roll.charges() {
            if let Some(p) = roll.recharge_progress() {
                draw_circle(c.x, c.y, r * p, gfx::with_alpha(LANTERN, 0.75));
            }
        }
    }
}

/// The stamina meter under the sprint slot, with a tick where being winded
/// ends.
fn stamina_meter(st: &Stamina, x: f32, y: f32) {
    gfx::rounded_rect(x - 1.5, y - 1.5, SLOT + 3.0, METER_H + 3.0, 4.5, INK);
    gfx::rounded_rect(x, y, SLOT, METER_H, 3.0, SPENT);
    let w = SLOT * st.energy();
    if w > 1.0 {
        let fill = if st.winded() {
            WINDED
        } else {
            gfx::mix(WINDED, LANTERN, (st.energy() / WINDED_UNTIL).min(1.0))
        };
        gfx::rounded_rect(x, y, w, METER_H, (w * 0.5).min(3.0), fill);
        draw_rectangle(
            x + 2.0,
            y + 1.5,
            (w - 4.0).max(0.0),
            2.0,
            gfx::with_alpha(WHITE, 0.35),
        );
    }
    if st.winded() {
        let tx = x + SLOT * WINDED_UNTIL;
        draw_rectangle(tx - 1.0, y - 2.0, 2.0, METER_H + 4.0, MOON);
    }
}

fn key_label(text: &str, bottom_centre: Vec2) {
    let size = 18;
    let dims = measure_text(text, None, size, 1.0);
    let x = (bottom_centre.x - dims.width * 0.5).round();
    let y = bottom_centre.y.round();
    draw_text(text, x + 1.0, y + 1.0, size as f32, INK);
    draw_text(text, x, y, size as f32, MOON);
}

fn centred_text(text: &str, c: Vec2, size: u16, color: Color) {
    let dims = measure_text(text, None, size, 1.0);
    let x = (c.x - dims.width * 0.5).round();
    let y = (c.y + dims.offset_y * 0.5).round();
    draw_text(text, x + 1.0, y + 1.0, size as f32, INK);
    draw_text(text, x, y, size as f32, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn run(secs: f32, mut f: impl FnMut()) {
        for _ in 0..(secs / DT).round() as usize {
            f();
        }
    }

    #[test]
    fn three_rolls_then_a_lockout_that_refills_all() {
        let mut r = RollCharges::default();
        for left in (0..ROLL_CHARGES).rev() {
            assert!(r.try_use());
            assert_eq!(r.charges(), left);
            run(0.4, || r.update(DT)); // back to back: one roll's length
        }
        assert!(r.lockout_left().is_some(), "spending the last charge locks");
        assert!(!r.try_use(), "no roll while locked out");
        run(ROLL_LOCKOUT_S + 0.05, || r.update(DT));
        assert_eq!(r.lockout_left(), None);
        assert_eq!(
            r.charges(),
            ROLL_CHARGES,
            "the lockout refills every charge"
        );
    }

    #[test]
    fn a_spent_charge_trickles_back_after_a_rest() {
        let mut r = RollCharges::default();
        assert!(r.try_use());
        run(ROLL_RECHARGE_S * 0.5, || r.update(DT));
        assert_eq!(r.charges(), ROLL_CHARGES - 1);
        assert!(r.recharge_progress().unwrap() > 0.4);
        run(ROLL_RECHARGE_S * 0.5 + 0.05, || r.update(DT));
        assert_eq!(r.charges(), ROLL_CHARGES);
        assert_eq!(r.recharge_progress(), None);
        assert_eq!(r.lockout_left(), None, "no lockout unless all are spent");
    }

    #[test]
    fn sprinting_drains_and_running_dry_winds_the_player() {
        let mut s = Stamina::default();
        run(SPRINT_S * 0.5, || s.update(DT, true));
        assert!((s.energy() - 0.5).abs() < 0.03);
        assert!(s.can_sprint());
        run(SPRINT_S * 0.5 + 0.1, || s.update(DT, true));
        assert_eq!(s.energy(), 0.0);
        assert!(s.winded() && !s.can_sprint());
        // Holding Shift while winded doesn't sprint or drain.
        s.update(DT, true);
        assert!(!s.sprinting());
    }

    #[test]
    fn stamina_recovers_after_a_delay_and_winded_ends_at_the_threshold() {
        let mut s = Stamina::default();
        s.exhaust();
        run(STAMINA_REGEN_DELAY_S * 0.9, || s.update(DT, false));
        assert_eq!(s.energy(), 0.0, "nothing comes back during the delay");
        let to_threshold = STAMINA_REFILL_S * WINDED_UNTIL;
        run(STAMINA_REGEN_DELAY_S * 0.1 + to_threshold * 0.9, || {
            s.update(DT, false)
        });
        assert!(s.winded(), "still winded below the threshold");
        run(to_threshold * 0.2, || s.update(DT, false));
        assert!(!s.winded() && s.can_sprint());
        run(STAMINA_REFILL_S, || s.update(DT, false));
        assert_eq!(s.energy(), 1.0, "refills and stops at full");
    }

    #[test]
    fn a_short_break_doesnt_refill_mid_sprint() {
        let mut s = Stamina::default();
        run(1.0, || s.update(DT, true));
        let e = s.energy();
        run(STAMINA_REGEN_DELAY_S * 0.8, || s.update(DT, false));
        assert_eq!(s.energy(), e);
    }
}
