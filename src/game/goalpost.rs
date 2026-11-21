//! The goalpost tracker's state (FR31): which goalpost is shown, how far
//! away it is, and the animation between goalposts. It holds no drawing code;
//! `ui::goalpost_panel` paints a [`TrackerView`] (invariant 5), and the tracker
//! changes no XP, speed or answer (invariant 7).
//!
//! The badge pops in as a circle, unrolls into a pill with the label and the
//! metres left, shows "Now!" when the player gets there, then closes back
//! into a circle and pops out before the next goalpost appears.

use super::world::{GoalKind, Goalpost, PX_PER_METER};

/// Seconds for the circle to pop in (and out).
pub const POP_S: f32 = 0.25;
/// Seconds for the circle to unroll into the pill (and roll back up).
pub const UNROLL_S: f32 = 0.30;
/// Seconds the "Now!" shows after the player reaches a goalpost.
pub const ARRIVED_S: f32 = 0.8;
/// The ring pulses once the goalpost is closer than this many metres.
pub const PULSE_UNDER_M: u32 = 10;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    /// Nothing shown; waiting for a goalpost.
    Idle,
    /// Popping in, then unrolling, then counting down.
    Showing,
    /// The player got there; "Now!" for `ARRIVED_S`.
    Arrived(f32),
    /// Rolling up and shrinking away, to be followed by the next goalpost
    /// or nothing.
    Leaving,
}

/// What the panel draws.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackerView {
    pub kind: GoalKind,
    pub label: String,
    /// Metres left, rounded up so it reads 0 on arrival, not before.
    pub meters: u32,
    /// 0..=1: the circle's pop-in.
    pub pop: f32,
    /// 0..=1: how far the circle has unrolled into the pill.
    pub open: f32,
    /// 0..=1: how much of the way from where this goalpost appeared to it
    /// the player has come.
    pub ring: f32,
    /// Under `PULSE_UNDER_M` and not yet reached.
    pub pulse: bool,
    pub arrived: bool,
    /// Animation clock for the pulse, on the game clock (invariant 8).
    pub clock: f32,
}

pub struct Tracker {
    phase: Phase,
    shown: Option<Goalpost>,
    /// Pixels from the player to the goalpost when it appeared, for the ring.
    start_px: f32,
    left_px: f32,
    pop: f32,
    open: f32,
    clock: f32,
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new()
    }
}

fn approach(v: f32, goal: f32, step: f32) -> f32 {
    if v < goal {
        (v + step).min(goal)
    } else {
        (v - step).max(goal)
    }
}

/// The same goalpost: same kind, standing in the same place.
fn same(a: &Goalpost, b: &Goalpost) -> bool {
    a.kind == b.kind && a.at == b.at
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            shown: None,
            start_px: 1.0,
            left_px: 0.0,
            pop: 0.0,
            open: 0.0,
            clock: 0.0,
        }
    }

    /// Advances one frame. `target` is what `World::next_goalpost` says is
    /// next for a player at `player_y`; `visible` is false while the tracker
    /// should be out of the way (a battle, a problem, the hub).
    pub fn update(&mut self, dt: f32, player_y: f32, target: Option<Goalpost>, visible: bool) {
        self.clock += dt;
        if let Some(g) = &self.shown {
            self.left_px = (player_y - g.y).max(0.0);
        }

        if !visible && self.phase != Phase::Idle {
            self.phase = Phase::Leaving;
        }
        match self.phase {
            Phase::Idle => {
                if let (true, Some(g)) = (visible, target) {
                    self.shown = Some(g);
                    self.start_px = (player_y - g.y).max(1.0);
                    self.left_px = self.start_px;
                    self.phase = Phase::Showing;
                }
            }
            Phase::Showing => {
                let g = self.shown.expect("showing implies a goalpost");
                if player_y <= g.y {
                    self.phase = Phase::Arrived(0.0);
                } else if !target.is_some_and(|t| same(&t, &g)) {
                    // Done early (a flask opened from the side) or replaced.
                    self.phase = Phase::Leaving;
                }
            }
            Phase::Arrived(t) => {
                let t = t + dt;
                self.phase = if t >= ARRIVED_S {
                    Phase::Leaving
                } else {
                    Phase::Arrived(t)
                };
            }
            Phase::Leaving => {}
        }

        match self.phase {
            Phase::Showing | Phase::Arrived(_) => {
                self.pop = approach(self.pop, 1.0, dt / POP_S);
                // Unrolls once the circle has popped in.
                let goal = if self.pop >= 1.0 { 1.0 } else { 0.0 };
                self.open = approach(self.open, goal, dt / UNROLL_S);
            }
            Phase::Leaving => {
                self.open = approach(self.open, 0.0, dt / UNROLL_S);
                if self.open <= 0.0 {
                    self.pop = approach(self.pop, 0.0, dt / POP_S);
                }
                if self.open <= 0.0 && self.pop <= 0.0 {
                    self.phase = Phase::Idle;
                    self.shown = None;
                }
            }
            Phase::Idle => {}
        }
    }

    /// What to draw, or `None` while nothing is on screen.
    pub fn view(&self) -> Option<TrackerView> {
        let g = self.shown.as_ref()?;
        if self.pop <= 0.0 {
            return None;
        }
        let arrived = matches!(self.phase, Phase::Arrived(_));
        let meters = (self.left_px / PX_PER_METER).ceil() as u32;
        Some(TrackerView {
            kind: g.kind,
            label: label(g),
            meters,
            pop: self.pop,
            open: self.open,
            ring: if arrived {
                1.0
            } else {
                (1.0 - self.left_px / self.start_px).clamp(0.0, 1.0)
            },
            pulse: !arrived && meters < PULSE_UNDER_M,
            arrived,
            clock: self.clock,
        })
    }
}

/// The name shown beside the circle.
pub fn label(g: &Goalpost) -> String {
    match g.kind {
        GoalKind::Milestone => format!("{} m mark", g.meters),
        GoalKind::Flask => "Potion flask".into(),
        GoalKind::Patch => "Rustling ferns".into(),
        GoalKind::Blocker => "Monster in the road".into(),
        GoalKind::Titan => "Tide Titan".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::prelude::vec2;

    fn goal(kind: GoalKind, y: f32) -> Goalpost {
        Goalpost {
            kind,
            y,
            at: vec2(0.0, y),
            meters: 0,
        }
    }

    /// Runs `secs` of frames at 60 fps with a fixed target.
    fn run(t: &mut Tracker, secs: f32, y: f32, target: Option<Goalpost>, visible: bool) {
        for _ in 0..(secs * 60.0) as usize {
            t.update(1.0 / 60.0, y, target, visible);
        }
    }

    #[test]
    fn a_new_goalpost_pops_in_as_a_circle_then_unrolls() {
        let g = goal(GoalKind::Titan, -2400.0);
        let mut t = Tracker::new();
        assert!(t.view().is_none());

        t.update(1.0 / 60.0, 0.0, Some(g), true);
        let v = t.view().unwrap();
        assert!(v.pop > 0.0 && v.open == 0.0, "starts as a circle: {v:?}");

        run(&mut t, POP_S, 0.0, Some(g), true);
        let v = t.view().unwrap();
        assert_eq!(v.pop, 1.0);
        assert!(v.open < 1.0, "unrolls after popping: {v:?}");

        run(&mut t, UNROLL_S + 0.1, 0.0, Some(g), true);
        assert_eq!(t.view().unwrap().open, 1.0);
    }

    #[test]
    fn the_count_goes_down_as_the_player_walks_north() {
        let g = goal(GoalKind::Milestone, -2400.0);
        let mut t = Tracker::new();
        run(&mut t, 1.0, 0.0, Some(g), true);
        assert_eq!(t.view().unwrap().meters, 100);
        run(&mut t, 0.1, -1200.0, Some(g), true);
        let v = t.view().unwrap();
        assert_eq!(v.meters, 50);
        assert!((v.ring - 0.5).abs() < 1e-3, "{v:?}");
        // Rounds up: 49.6 m left reads 50, and only the arrival reads 0.
        run(&mut t, 0.1, -1209.0, Some(g), true);
        assert_eq!(t.view().unwrap().meters, 50);
    }

    #[test]
    fn it_pulses_when_close_and_says_now_on_arrival() {
        let g = goal(GoalKind::Flask, -2400.0);
        let mut t = Tracker::new();
        run(&mut t, 1.0, 0.0, Some(g), true);
        assert!(!t.view().unwrap().pulse);
        run(&mut t, 0.1, g.y + 5.0 * PX_PER_METER, Some(g), true);
        assert!(t.view().unwrap().pulse);

        run(&mut t, 0.1, g.y - 1.0, Some(g), true);
        let v = t.view().unwrap();
        assert!(
            v.arrived && v.meters == 0 && v.ring == 1.0 && !v.pulse,
            "{v:?}"
        );

        // Then it closes and goes; with nothing next it stays gone.
        run(
            &mut t,
            ARRIVED_S + UNROLL_S + POP_S + 0.2,
            g.y - 1.0,
            None,
            true,
        );
        assert!(t.view().is_none());
    }

    #[test]
    fn it_closes_before_the_next_goalpost_appears() {
        let a = goal(GoalKind::Flask, -2400.0);
        let b = goal(GoalKind::Patch, -4800.0);
        let mut t = Tracker::new();
        run(&mut t, 1.0, 0.0, Some(a), true);
        // `a` is gone (solved), `b` is next.
        run(&mut t, 0.05, 0.0, Some(b), true);
        let v = t.view().unwrap();
        assert_eq!(v.kind, GoalKind::Flask, "still the old one while it closes");
        assert!(v.open < 1.0, "closing, not jumping: {v:?}");
        run(&mut t, UNROLL_S + POP_S + 0.1, 0.0, Some(b), true);
        assert_eq!(t.view().unwrap().kind, GoalKind::Patch);
    }

    #[test]
    fn it_hides_while_a_battle_or_problem_is_open_and_comes_back() {
        let g = goal(GoalKind::Titan, -2400.0);
        let mut t = Tracker::new();
        run(&mut t, 1.0, 0.0, Some(g), true);
        run(&mut t, UNROLL_S + POP_S + 0.1, 0.0, Some(g), false);
        assert!(t.view().is_none());
        run(&mut t, 1.0, 0.0, Some(g), true);
        assert_eq!(t.view().unwrap().open, 1.0);
    }

    #[test]
    fn nothing_ahead_shows_nothing() {
        let mut t = Tracker::new();
        run(&mut t, 1.0, 0.0, None, true);
        assert!(t.view().is_none());
    }

    #[test]
    fn labels_name_each_kind() {
        let mut m = goal(GoalKind::Milestone, 0.0);
        m.meters = 300;
        assert_eq!(label(&m), "300 m mark");
        assert_eq!(label(&goal(GoalKind::Titan, 0.0)), "Tide Titan");
    }
}
