//! The hero's bow (FR19). It rides slung across their back; F raises it,
//! draws the string and looses an arrow that arcs across the ground and
//! sticks where it lands (or splashes, over the lake). In a battle the same
//! shot is the player's attack.
//!
//! Like jump and roll, the shot is cosmetic (invariant 7): an arrow never
//! hits, hurts or blocks anything, and a battle's result still comes only
//! from the answer.

use super::gfx::{self, INK};
use super::world::{self, View};
use macroquad::prelude::*;
use std::f32::consts::PI;

/// The whole shot: raise, draw, loose, follow through, lower.
pub const SHOOT_S: f32 = 0.55;
/// The bow comes up from the hip to arm's length.
const RAISE_S: f32 = 0.08;
/// The string is let go and the arrow leaves.
pub const RELEASE_S: f32 = 0.26;
/// The last stretch of the shot lowers the bow again.
const LOWER_S: f32 = 0.12;

/// Arrow speed across the ground, px/s.
const ARROW_SPEED: f32 = 560.0;
/// The arrow leaves slightly upward, then gravity brings it down: about a
/// third of a screen of flight from hand height.
const ARROW_RISE: f32 = 40.0;
const GRAVITY: f32 = 260.0;
/// A landed arrow stays stuck in the road or ground this long, fading out
/// over the last `FADE_S`.
const STUCK_S: f32 = 1.4;
const FADE_S: f32 = 0.4;
/// The ripple an arrow makes when it lands in the lake.
const SPLASH_S: f32 = 0.5;
/// Oldest arrows are dropped past this many.
pub const MAX_ARROWS: usize = 8;
/// Arrow length in the sprite's units.
pub const ARROW_LEN: f32 = 22.0;
/// Where an arrow leaves from, relative to the archer's feet (x is
/// multiplied by the facing).
pub const NOCK_FROM_FOOT: Vec2 = Vec2::new(14.0, -18.0);

pub const WOOD: Color = Color::new(0.72, 0.45, 0.22, 1.0);
const STRING: Color = Color::new(0.98, 0.94, 0.82, 1.0);
const STEEL: Color = Color::new(0.80, 0.84, 0.88, 1.0);
const FLETCH: Color = Color::new(0.92, 0.32, 0.30, 1.0);

fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// The shot's pose `t` seconds in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    /// 0 with the bow at the hip, 1 held out at arm's length.
    pub raise: f32,
    /// 0 with the string at rest, 1 pulled to full draw.
    pub draw: f32,
    /// An arrow is on the string (before the release).
    pub nocked: bool,
    /// The string's twang after the release, in px (decays to 0).
    pub snap: f32,
}

pub fn aim(t: f32) -> Aim {
    let raise = if t < RAISE_S {
        ease(t / RAISE_S)
    } else if t > SHOOT_S - LOWER_S {
        1.0 - ease((t - (SHOOT_S - LOWER_S)) / LOWER_S)
    } else {
        1.0
    };
    let nocked = t < RELEASE_S;
    let draw = if nocked {
        ease((t - RAISE_S) / (RELEASE_S - RAISE_S))
    } else {
        0.0
    };
    let snap = if nocked {
        0.0
    } else {
        let s = t - RELEASE_S;
        (s * 70.0).sin() * (-s * 14.0).exp() * 3.0
    };
    Aim {
        raise,
        draw,
        nocked,
        snap,
    }
}

/// Points along a bow held upright with its grip at `grip`, facing `dir`
/// (+1 right): the limbs curve back toward the archer, and further the
/// harder the string is drawn.
fn limb_points(grip: Vec2, dir: f32, draw: f32) -> [Vec2; 9] {
    let half = 14.0 * (1.0 - 0.08 * draw);
    let bend = 5.0 + 2.5 * draw;
    std::array::from_fn(|i| {
        let s = i as f32 / 4.0 - 1.0;
        grip + vec2(-dir * bend * s * s, s * half)
    })
}

/// Where the string's middle (and the arrow's tail) sits for this aim.
pub fn nock_point(grip: Vec2, dir: f32, aim: &Aim) -> Vec2 {
    let tips = limb_points(grip, dir, aim.draw);
    vec2(tips[0].x - dir * (9.0 * aim.draw - aim.snap), grip.y)
}

/// The bow held out to shoot, in the sprite's units, with an arrow on the
/// string until the release. The hands are the sprite's to draw.
pub fn draw_held(grip: Vec2, dir: f32, aim: &Aim) {
    let limbs = limb_points(grip, dir, aim.draw);
    let nock = nock_point(grip, dir, aim);
    gfx::polyline(&[limbs[0], nock, limbs[8]], 1.0, STRING);
    gfx::polyline(&limbs, 4.2, INK);
    gfx::polyline(&limbs, 2.4, WOOD);
    // Leather wrap at the grip.
    gfx::line(grip - vec2(0.0, 2.5), grip + vec2(0.0, 2.5), 3.2, INK);
    if aim.nocked {
        draw_arrow(nock, vec2(dir, 0.0), ARROW_LEN, 1.0, 1.0, true);
    }
}

/// The bow at rest, slung diagonally across the back: drawn before the
/// body so only its tips peek out past the shirt and shorts.
pub fn draw_slung(body: Vec2) {
    // The top limb clears the head past the right shoulder; the bottom one
    // shows below the left hip.
    let (top, bottom) = (body + vec2(20.0, -17.0), body + vec2(-16.0, 11.0));
    let axis = bottom - top;
    let bulge = -axis.perp().normalize_or_zero() * 4.0;
    let limbs: Vec<Vec2> = (0..=8)
        .map(|i| {
            let s = i as f32 / 8.0;
            top + axis * s + bulge * (s * PI).sin()
        })
        .collect();
    gfx::line(top, bottom, 1.0, STRING);
    gfx::polyline(&limbs, 4.2, INK);
    gfx::polyline(&limbs, 2.4, WOOD);
}

/// One arrow, its tail at `tail`, pointing along unit `along`. `len` and
/// `thick` scale it (the battle view draws it bigger); `head` is false for
/// an arrow with its point buried in the ground.
pub fn draw_arrow(tail: Vec2, along: Vec2, len: f32, thick: f32, alpha: f32, head: bool) {
    let side = along.perp();
    let tip = tail + along * len;
    let ink = gfx::with_alpha(INK, INK.a * alpha);
    gfx::line(tail, tip, 3.0 * thick, ink);
    gfx::line(tail, tip, 1.6 * thick, gfx::with_alpha(WOOD, alpha));
    if head {
        let (point, a, b) = (
            tip + along * 4.5 * thick,
            tip + side * 2.8 * thick,
            tip - side * 2.8 * thick,
        );
        draw_triangle(
            point + along * thick,
            a - along * 0.6 * thick,
            b - along * 0.6 * thick,
            ink,
        );
        draw_triangle(point, a, b, gfx::with_alpha(STEEL, alpha));
    }
    // Two red feathers swept back from the tail.
    for s in [-1.0, 1.0] {
        let root = tail + along * 1.0 * thick;
        gfx::quad(
            root,
            root + along * 5.0 * thick,
            root + along * 2.0 * thick + side * s * 2.6 * thick,
            root - along * 1.5 * thick + side * s * 2.8 * thick,
            gfx::with_alpha(FLETCH, alpha),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ArrowState {
    Flying,
    /// Seconds since it stuck in the ground, and the angle it landed at.
    Stuck(f32, Vec2),
    /// Seconds since it dropped into the lake.
    Splash(f32),
}

/// An arrow loosed on the road. Purely for show: nothing collides with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrow {
    /// The point on the ground under the arrow (world coordinates).
    pub ground: Vec2,
    /// Height above the ground, in px (the road is `ROAD_LIFT` up).
    z: f32,
    vx: f32,
    vz: f32,
    state: ArrowState,
}

/// How high the surface under `p` is: the road's lift on the road,
/// ground level off it.
fn surface_lift(p: Vec2) -> f32 {
    if (p.x - world::path_center(p.y)).abs() < world::PATH_HALF_W {
        world::ground_lift(p.y)
    } else {
        0.0
    }
}

impl Arrow {
    /// Loosed from `ground` at `z` px above the ground, flying `dir` (+1
    /// right).
    pub fn new(ground: Vec2, z: f32, dir: f32) -> Self {
        Self {
            ground,
            z,
            vx: dir * ARROW_SPEED,
            vz: ARROW_RISE,
            state: ArrowState::Flying,
        }
    }

    pub fn flying(&self) -> bool {
        self.state == ArrowState::Flying
    }

    /// True once it has faded out or its splash has settled.
    pub fn done(&self) -> bool {
        match self.state {
            ArrowState::Flying => false,
            ArrowState::Stuck(s, _) => s >= STUCK_S,
            ArrowState::Splash(s) => s >= SPLASH_S,
        }
    }

    pub fn update(&mut self, dt: f32) {
        match &mut self.state {
            ArrowState::Flying => {
                self.ground.x += self.vx * dt;
                self.z += self.vz * dt;
                self.vz -= GRAVITY * dt;
                let floor = surface_lift(self.ground);
                if self.z <= floor {
                    self.z = floor;
                    self.state = if self.ground.x < world::bank_x(self.ground.y) {
                        ArrowState::Splash(0.0)
                    } else {
                        // Flights land shallow; tilt it into the ground so it
                        // reads as stuck, not dropped.
                        ArrowState::Stuck(0.0, vec2(self.vx.signum() * 0.8, 0.6))
                    };
                }
            }
            ArrowState::Stuck(s, _) | ArrowState::Splash(s) => *s += dt,
        }
    }

    /// Unit direction of flight on screen (screen y is down, height up).
    fn along(&self) -> Vec2 {
        vec2(self.vx, -self.vz).normalize_or(vec2(1.0, 0.0))
    }

    pub fn draw(&self, view: &View) {
        let floor = view.to_screen(self.ground) - vec2(0.0, surface_lift(self.ground));
        match self.state {
            ArrowState::Flying => {
                let up = self.z - surface_lift(self.ground);
                let k = 1.0 - (up / 60.0).clamp(0.0, 0.6);
                gfx::ellipse(floor, 9.0 * k, 2.2 * k, gfx::SHADOW);
                let along = self.along();
                let mid = view.to_screen(self.ground) - vec2(0.0, self.z);
                draw_arrow(
                    mid - along * ARROW_LEN * 0.5,
                    along,
                    ARROW_LEN,
                    1.0,
                    1.0,
                    true,
                );
            }
            ArrowState::Stuck(s, along) => {
                let alpha = (1.0 - (s - (STUCK_S - FADE_S)) / FADE_S).clamp(0.0, 1.0);
                // Point buried: only the back half and the feathers show.
                let shown = ARROW_LEN * 0.6;
                gfx::ellipse(
                    floor,
                    4.0,
                    1.4,
                    gfx::with_alpha(gfx::SHADOW, gfx::SHADOW.a * alpha),
                );
                draw_arrow(floor - along * shown, along, shown, 1.0, alpha, false);
            }
            ArrowState::Splash(s) => {
                let u = s / SPLASH_S;
                let foam = Color::new(0.90, 0.97, 1.0, 0.85 * (1.0 - u));
                gfx::ellipse_outline(floor, 4.0 + u * 14.0, 1.6 + u * 5.0, 1.6, foam);
                for i in 0..3 {
                    let a = PI * (0.25 + 0.25 * i as f32);
                    let hop = (u * PI).sin() * 9.0;
                    let p = floor + vec2(a.cos() * u * 10.0, -a.sin() * hop);
                    draw_circle(p.x, p.y, 1.6 * (1.0 - u), foam);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shot_draws_then_releases_then_lowers() {
        let start = aim(0.0);
        assert_eq!(start.raise, 0.0);
        assert!(start.nocked);
        let drawn = aim(RELEASE_S - 0.001);
        assert!(drawn.nocked && drawn.draw > 0.95 && drawn.raise == 1.0);
        let loosed = aim(RELEASE_S + 0.01);
        assert!(!loosed.nocked && loosed.draw == 0.0);
        assert!(aim(SHOOT_S).raise < 1e-4, "the bow is lowered by the end");
    }

    #[test]
    fn an_arrow_flies_the_way_it_was_loosed_then_lands_and_fades() {
        let y = world::ROAD_START_Y - 2000.0;
        let from = vec2(world::path_center(y), y);
        for dir in [-1.0, 1.0] {
            let mut a = Arrow::new(from, world::ROAD_LIFT + 18.0, dir);
            let mut t = 0.0;
            while a.flying() && t < 3.0 {
                a.update(1.0 / 60.0);
                t += 1.0 / 60.0;
            }
            assert!(!a.flying(), "it comes down");
            let flown = (a.ground.x - from.x) * dir;
            assert!(flown > 150.0 && flown < 500.0, "flew {flown} px");
            assert_eq!(a.ground.y, from.y, "it flies across, not along");
            while !a.done() && t < 6.0 {
                a.update(1.0 / 60.0);
                t += 1.0 / 60.0;
            }
            assert!(a.done(), "and goes away");
        }
    }
}
