//! Draws a data-defined monster (FR21): one renderer for every
//! `MonsterSpec`, built from the same flat primitives and `INK` outlines as
//! the hand-drawn roster in `monster.rs`. It's called from inside
//! `monster::draw`, so a spec gets the shared canvas, poses, hurt flash,
//! faint fade and reduce-flashing handling for free. It only draws the
//! sprite itself.
//!
//! Coordinates are canvas pixels with `o` the ground point under the
//! monster. The monster faces left, toward the player, so "front" is −x.

use super::gfx::{self, INK};
use super::monster;
use crate::content::monster_spec::{
    Accessory, Attack, BodyShape, EyeLook, Idle, MonsterSpec, Part, PartKind, Rgb, Side, Special,
};
use macroquad::prelude::*;
use std::f32::consts::{PI, TAU};

/// How far the rig may reach from the ground point, in canvas pixels: it
/// has to stay inside `monster::MonsterCanvas`.
const MAX_UP: f32 = 88.0;
const MAX_SIDE: f32 = 56.0;

fn col(c: Rgb) -> Color {
    Color::new(c.0[0], c.0[1], c.0[2], 1.0)
}

/// Where things attach on the body this frame.
struct Geo {
    /// Centre and half-size of the body ellipse (approximate for the
    /// shell and segmented shapes).
    c: Vec2,
    rx: f32,
    ry: f32,
    /// Centre of the eyes.
    face: Vec2,
    /// Top of the head, for horns and accessories.
    top: Vec2,
}

impl Geo {
    /// A point on the body's outline, `deg` degrees anticlockwise from
    /// straight back (0 = back, 90 = up, 180 = front, 270 = down), and the
    /// outward direction there.
    fn rim(&self, deg: f32) -> (Vec2, Vec2) {
        let a = deg.to_radians();
        let d = vec2(a.cos(), -a.sin());
        (self.c + vec2(d.x * self.rx, d.y * self.ry), d)
    }
}

/// Spreads `n` values evenly across `lo..=hi` (the middle when `n` is 1).
fn spread(n: u32, lo: f32, hi: f32) -> impl Iterator<Item = f32> {
    (0..n).map(move |i| {
        if n == 1 {
            (lo + hi) * 0.5
        } else {
            lo + (hi - lo) * i as f32 / (n - 1) as f32
        }
    })
}

/// The angle ranges (degrees, see [`Geo::rim`]) a part spreads over for
/// each side.
fn arcs(side: Side, front: (f32, f32), back: (f32, f32)) -> Vec<(f32, f32)> {
    match side {
        Side::Front => vec![front],
        Side::Back => vec![back],
        Side::Both => vec![front, back],
    }
}

fn has(spec: &MonsterSpec, kind: PartKind) -> Option<&Part> {
    spec.parts.iter().find(|p| p.kind == kind)
}

/// Draws `spec` standing on `o`. `atk` is the attack's swell (0..1..0) and
/// `atk_u` its raw progress (0..1, 0 when not attacking); `hurt` is 1 when
/// just hit, fading to 0. `grown` is the wrong answers so far, which puff
/// up a monster with the grows special, and `has_hat` puts the player's
/// stolen hat on its head.
#[allow(clippy::too_many_arguments)]
pub fn draw_spec(
    o: Vec2,
    spec: &MonsterSpec,
    t: f32,
    atk: f32,
    atk_u: f32,
    hurt: f32,
    grown: u32,
    has_hat: bool,
) {
    let main = col(spec.palette.main);
    let accent = col(spec.palette.accent);
    let belly = col(spec.palette.belly);
    let s_max = spec.parts.iter().map(|p| p.size).fold(1.0f32, f32::max);

    // What it stands on: legs or tentacles lift the body off the ground.
    let leg_len = has(spec, PartKind::Leg).map_or(0.0, |p| 9.0 * p.size);
    let tent_len = has(spec, PartKind::Tentacle).map_or(0.0, |p| 16.0 * p.size);
    let mut lift = leg_len.max(tent_len * 0.8);
    if spec.idle == Idle::Float {
        lift += 12.0 + (t * 2.0).sin() * 5.0;
    }

    // Size: the spec's body, puffed by the grows special and the pulse
    // idle, then fitted to the canvas.
    let grow = if spec.special == Special::Grows {
        (1.0 + grown as f32 * 0.15).min(1.5)
    } else {
        1.0
    };
    let pulse = if spec.idle == Idle::Pulse {
        1.0 + (t * 3.0).sin() * 0.06
    } else {
        1.0
    };
    let mut rx = spec.body.w * 0.5 * grow * pulse;
    let mut ry = spec.body.h * 0.5 * grow * pulse;
    let headroom = 14.0 * s_max + if spec.eyes.stalks { 12.0 } else { 0.0 };
    let fit = ((MAX_UP - lift - headroom) / (2.0 * ry))
        .min((MAX_SIDE - 14.0 * s_max) / rx)
        .clamp(0.4, 1.0);
    rx *= fit;
    ry *= fit;

    // Idle and attack motion.
    let (mut dx, mut dy, mut squash_x) = (0.0f32, 0.0f32, 1.0f32);
    match spec.idle {
        Idle::Bob => dy -= ((t * 2.4).sin() * 0.5 + 0.5) * 3.0,
        Idle::Scuttle => dx += (t * 2.2).sin() * 5.0,
        Idle::Sway => dx += (t * 1.5).sin() * 3.5,
        Idle::Float | Idle::Pulse => {}
    }
    let slam_rise = 26.0;
    match spec.attack {
        Attack::Lunge => dx -= atk * 10.0,
        Attack::Slam if atk_u > 0.0 => {
            // Up, hang, then down hard.
            dy -= if atk_u < 0.4 {
                slam_rise * (atk_u / 0.4)
            } else {
                slam_rise * (1.0 - (atk_u - 0.4) / 0.1).max(0.0)
            };
        }
        Attack::Spin if atk_u > 0.0 => {
            squash_x = 0.55 + 0.45 * (atk_u * 3.0 * PI).cos().abs();
        }
        _ => {}
    }

    let c = o + vec2(dx, dy - lift - ry);
    let body_rx = rx * squash_x;
    let geo = match spec.body.shape {
        BodyShape::Blob => Geo {
            c,
            rx: body_rx,
            ry,
            face: c + vec2(-body_rx * 0.35, -ry * 0.25),
            top: c + vec2(-body_rx * 0.2, -ry),
        },
        BodyShape::Shell => {
            let head = c + vec2(-body_rx * 0.45, ry * 0.25);
            Geo {
                c,
                rx: body_rx,
                ry,
                face: head + vec2(-body_rx * 0.1, -ry * 0.2),
                top: head + vec2(0.0, -ry * 0.5),
            }
        }
        BodyShape::Dome => Geo {
            c,
            rx: body_rx,
            ry,
            face: c + vec2(-body_rx * 0.3, ry * 0.1),
            top: c + vec2(-body_rx * 0.1, -ry),
        },
        BodyShape::Segmented => {
            let r = ry * 0.8;
            let head = c + vec2(-body_rx + r, -ry * 0.15);
            Geo {
                c,
                rx: body_rx,
                ry,
                face: head + vec2(-r * 0.2, -r * 0.2),
                top: head + vec2(0.0, -r),
            }
        }
        BodyShape::Wave => Geo {
            c,
            rx: body_rx,
            ry,
            face: c + vec2(-body_rx * 0.25, -ry * 0.05),
            top: c + vec2(-body_rx * 0.2, -ry * 1.05),
        },
        BodyShape::SpikyBall => {
            let r = body_rx.min(ry);
            Geo {
                c,
                rx: r,
                ry: r,
                face: c + vec2(-r * 0.35, -r * 0.2),
                top: c + vec2(-r * 0.15, -r),
            }
        }
    };

    // The zap's halo goes behind everything.
    if spec.attack == Attack::Zap && atk > 0.25 {
        let r = (geo.rx.max(geo.ry) + 14.0) * atk;
        draw_circle(geo.c.x, geo.c.y, r, gfx::with_alpha(accent, 0.35 * atk));
        for i in 0..6 {
            let a = i as f32 / 6.0 * TAU + t * 4.0;
            let p = geo.c + vec2(a.cos(), a.sin()) * r;
            gfx::star(p, 4.0, 2.0, 5, a, 1.0, Color::new(1.0, 0.9, 0.3, 1.0));
        }
    }

    // Parts behind the body.
    for part in &spec.parts {
        match part.kind {
            PartKind::Tail => draw_tails(&geo, part, main, t, atk),
            PartKind::Wing => draw_wings(&geo, part, accent, t, atk),
            PartKind::Fin => draw_fins(&geo, part, accent, t),
            PartKind::Spike => draw_spikes(&geo, part, accent, atk),
            PartKind::Leg => draw_legs(&geo, o, part, main, t, spec.idle == Idle::Scuttle),
            PartKind::Tentacle => draw_tentacles(&geo, part, main, t),
            PartKind::Claw | PartKind::Horn => {}
        }
    }

    draw_body(spec.body.shape, &geo, main, accent, belly, t, hurt);

    // Parts in front of the body.
    for part in &spec.parts {
        match part.kind {
            PartKind::Claw => draw_claws(&geo, part, accent, t, atk),
            PartKind::Horn => draw_horns(&geo, part, belly),
            _ => {}
        }
    }

    // The face.
    let eye_xs: Vec<f32> = {
        let n = spec.eyes.count;
        let gap = spec.eyes.size * 2.5;
        (0..n)
            .map(|i| geo.face.x + (i as f32 - (n - 1) as f32 * 0.5) * gap)
            .collect()
    };
    let eye_y = |i: usize| {
        let mid = spec.eyes.count == 3 && i == 1;
        geo.face.y - if mid { spec.eyes.size * 0.8 } else { 0.0 }
    };
    let size = spec.eyes.size;
    if spec.accessory == Accessory::Mask {
        let (l, r) = (
            eye_xs[0] - size * 1.7,
            eye_xs[eye_xs.len() - 1] + size * 1.7,
        );
        let y = eye_y(0);
        gfx::rounded_rect(l, y - size * 1.2, r - l, size * 2.4, size * 0.8, INK);
    }
    if spec.accessory == Accessory::Helmet {
        draw_helmet(&geo, size);
    }
    let shut = hurt > 0.2;
    for (i, &x) in eye_xs.iter().enumerate() {
        let mut p = vec2(x, eye_y(i));
        if spec.eyes.stalks {
            let top = p + vec2((t * 3.0 + i as f32).sin() * 1.5, -12.0 - size);
            gfx::line(p, top, 2.4, INK);
            gfx::line(p, top, 1.3, main);
            p = top;
        }
        eye(p, size, spec.eyes.look, main, shut);
    }
    let mouth = geo.face + vec2(-size * 0.6, size * 2.0 + 3.0);
    let open = match spec.attack {
        Attack::Lunge | Attack::Spit => atk,
        _ => 0.0,
    };
    if open > 0.15 {
        gfx::ellipse(mouth, 3.0 + open * 3.0, 1.5 + open * 4.0, INK);
    } else {
        match spec.eyes.look {
            EyeLook::Cute => draw_arc(mouth.x, mouth.y - 3.0, 12, 4.0, 20.0, 1.5, 140.0, INK),
            EyeLook::Grumpy => draw_arc(mouth.x, mouth.y + 2.0, 12, 4.0, 200.0, 1.5, 140.0, INK),
            EyeLook::Sleepy => draw_circle_lines(mouth.x, mouth.y, 1.8, 1.3, INK),
        }
    }

    match spec.accessory {
        Accessory::Crown => draw_crown(geo.top),
        Accessory::BowTie => draw_bow_tie(mouth + vec2(size * 0.8, size * 1.5 + 7.0), accent),
        Accessory::Leaf => draw_leaf(geo.top, t),
        Accessory::None | Accessory::Mask | Accessory::Helmet => {}
    }
    if has_hat {
        monster::stolen_hat(geo.top + vec2(2.0, -5.0), -12.0);
    }

    // Attack effects in front.
    match spec.attack {
        Attack::Spit if (0.25..0.8).contains(&atk_u) => {
            let k = (atk_u - 0.25) / 0.55;
            let p = mouth + vec2(-95.0 * k, -16.0 * (k * PI).sin());
            draw_circle(p.x, p.y, 4.5, accent);
            draw_circle_lines(p.x, p.y, 4.5, 1.3, INK);
        }
        Attack::Slam if (0.45..0.95).contains(&atk_u) => {
            let k = (atk_u - 0.45) / 0.5;
            for side in [-1.0f32, 1.0] {
                for j in 0..3 {
                    let p = o + vec2(
                        side * (geo.rx * 0.6 + k * 22.0 + j as f32 * 6.0),
                        -3.0 - j as f32 * 2.0,
                    );
                    draw_circle(
                        p.x,
                        p.y,
                        3.5 * (1.0 - k) + 1.0,
                        gfx::with_alpha(gfx::MOON, 0.5 * (1.0 - k)),
                    );
                }
            }
        }
        Attack::Spin if atk_u > 0.0 => {
            let r = geo.rx.max(geo.ry) + 7.0;
            for k in 0..2 {
                let rot = atk_u * 900.0 + k as f32 * 180.0;
                draw_arc(
                    geo.c.x,
                    geo.c.y,
                    20,
                    r,
                    rot,
                    2.0,
                    80.0,
                    gfx::with_alpha(gfx::MOON, 0.7 * atk),
                );
            }
        }
        _ => {}
    }
}

fn draw_body(
    shape: BodyShape,
    g: &Geo,
    main: Color,
    accent: Color,
    belly: Color,
    t: f32,
    hurt: f32,
) {
    let (c, rx, ry) = (g.c, g.rx, g.ry);
    let shine = Color::new(1.0, 1.0, 1.0, 0.35);
    match shape {
        BodyShape::Blob => {
            gfx::ellipse(c, rx + 1.8, ry + 1.8, INK);
            gfx::ellipse(c, rx, ry, main);
            gfx::ellipse(c + vec2(-rx * 0.15, ry * 0.4), rx * 0.6, ry * 0.42, belly);
            gfx::ellipse(c + vec2(-rx * 0.4, -ry * 0.55), rx * 0.2, ry * 0.12, shine);
        }
        BodyShape::Shell => {
            // A soft body in front, the shell on its back; it ducks into
            // the shell when hit.
            let duck = if hurt > 0.2 { ry * 0.3 } else { 0.0 };
            let b = c + vec2(-rx * 0.3, ry * 0.45 + duck);
            gfx::ellipse(b, rx * 0.75 + 1.8, ry * 0.5 + 1.8, INK);
            gfx::ellipse(b, rx * 0.75, ry * 0.5, main);
            gfx::ellipse(b + vec2(0.0, ry * 0.25), rx * 0.55, ry * 0.18, belly);
            let sc = c + vec2(rx * 0.2, -ry * 0.05);
            let (srx, sry) = (rx * 0.8, ry * 0.85);
            gfx::ellipse(sc, srx + 1.8, sry + 1.8, INK);
            gfx::ellipse(sc, srx, sry, accent);
            let pts: Vec<Vec2> = (0..28)
                .map(|i| {
                    let a = i as f32 * 0.42;
                    let k = 0.8 * (1.0 - i as f32 / 30.0);
                    sc + vec2(a.cos() * srx * k, a.sin() * sry * k)
                })
                .collect();
            gfx::polyline(&pts, 1.5, gfx::shade(accent, 0.7));
        }
        BodyShape::Dome => {
            let skirt = c + vec2(0.0, ry * 0.7);
            gfx::ellipse(skirt, rx + 2.8, ry * 0.32 + 1.8, INK);
            gfx::ellipse(skirt, rx + 1.0, ry * 0.32, gfx::shade(main, 0.85));
            gfx::ellipse(c, rx + 1.8, ry + 1.8, INK);
            gfx::ellipse(c, rx, ry, main);
            gfx::ellipse(c + vec2(0.0, ry * 0.55), rx * 0.8, ry * 0.25, belly);
            gfx::ellipse(c + vec2(-rx * 0.35, -ry * 0.5), rx * 0.28, ry * 0.16, shine);
        }
        BodyShape::Segmented => {
            let r = ry * 0.8;
            let n = ((rx * 2.0 / r).round() as usize).clamp(3, 6);
            let (x0, x1) = (c.x - rx + r, c.x + rx - r * 0.55);
            // Back to front, so the head (first) ends up on top.
            for i in (0..n).rev() {
                let k = i as f32 / (n - 1) as f32;
                let ri = r * (1.0 - 0.4 * k);
                let p = vec2(
                    x0 + (x1 - x0) * k,
                    c.y + ry - ri + (t * 3.0 - k * 3.0).sin() * 2.0
                        - if i == 0 { ry * 0.15 } else { 0.0 },
                );
                draw_circle(p.x, p.y, ri + 1.8, INK);
                draw_circle(
                    p.x,
                    p.y,
                    ri,
                    if i % 2 == 0 {
                        main
                    } else {
                        gfx::shade(main, 1.12)
                    },
                );
                gfx::ellipse(p + vec2(0.0, ri * 0.5), ri * 0.6, ri * 0.25, belly);
            }
        }
        BodyShape::Wave => {
            // Stacked swells narrowing up to a curling crest.
            let base = c + vec2(0.0, ry);
            for i in 0..5 {
                let k = i as f32 / 4.0;
                let p = base + vec2((t * 1.5).sin() * 3.0 * k, -ry * 0.2 - k * ry * 1.6);
                let w = rx * (1.0 - 0.45 * k);
                let h = ry * 0.32;
                gfx::ellipse(p, w + 1.8, h + 1.8, INK);
                gfx::ellipse(
                    p,
                    w,
                    h,
                    if i % 2 == 0 {
                        main
                    } else {
                        gfx::shade(main, 0.85)
                    },
                );
            }
            let crest = base + vec2((t * 1.5).sin() * 3.0 - rx * 0.15, -ry * 2.0);
            gfx::ellipse(crest, rx * 0.5 + 1.6, ry * 0.2 + 1.6, INK);
            gfx::ellipse(crest, rx * 0.5, ry * 0.2, belly);
            for i in 0..4 {
                let p = crest
                    + vec2(
                        -rx * 0.4 + i as f32 * rx * 0.27,
                        ry * 0.15 + (t * 5.0 + i as f32).sin() * 1.5,
                    );
                draw_circle(p.x, p.y, 2.4, belly);
            }
        }
        BodyShape::SpikyBall => {
            let r = rx.min(ry);
            for i in 0..14 {
                let a = i as f32 / 14.0 * TAU + (t * 0.5).sin() * 0.05;
                let d = vec2(a.cos(), a.sin());
                let n = vec2(-d.y, d.x);
                draw_triangle(
                    c + d * (r - 1.0) + n * 3.5,
                    c + d * (r - 1.0) - n * 3.5,
                    c + d * (r + 7.0),
                    gfx::shade(accent, 0.8),
                );
            }
            draw_circle(c.x, c.y, r + 1.8, INK);
            draw_circle(c.x, c.y, r, main);
            gfx::ellipse(c + vec2(r * 0.05, r * 0.45), r * 0.65, r * 0.35, belly);
            gfx::ellipse(c + vec2(-r * 0.4, -r * 0.55), r * 0.22, r * 0.12, shine);
        }
    }
}

/// One eye looking left, toward the player. `shut` squeezes it into ">"
/// when hurt.
fn eye(p: Vec2, r: f32, look: EyeLook, lid: Color, shut: bool) {
    if shut {
        gfx::line(p + vec2(-r, -r * 0.7), p + vec2(r * 0.6, 0.0), 1.6, INK);
        gfx::line(p + vec2(-r, r * 0.7), p + vec2(r * 0.6, 0.0), 1.6, INK);
        return;
    }
    draw_circle(p.x, p.y, r, WHITE);
    draw_circle(p.x - r * 0.35, p.y + r * 0.1, r * 0.55, INK);
    draw_circle(p.x - r * 0.5, p.y - r * 0.25, r * 0.2, WHITE);
    match look {
        EyeLook::Cute => {}
        EyeLook::Grumpy => {
            // A heavy brow slanting down toward the middle of the face.
            draw_triangle(
                p + vec2(-r * 1.1, -r * 0.1),
                p + vec2(r * 1.1, -r * 1.2),
                p + vec2(-r * 1.1, -r * 1.2),
                lid,
            );
            gfx::line(
                p + vec2(-r * 1.1, -r * 0.1),
                p + vec2(r * 1.1, -r * 0.9),
                1.6,
                INK,
            );
        }
        EyeLook::Sleepy => {
            // The upper lid drooping over half the eye.
            for i in 0..8 {
                let a0 = PI + i as f32 / 8.0 * PI;
                let a1 = PI + (i + 1) as f32 / 8.0 * PI;
                draw_triangle(
                    p,
                    p + vec2(a0.cos(), a0.sin()) * (r + 0.4),
                    p + vec2(a1.cos(), a1.sin()) * (r + 0.4),
                    lid,
                );
            }
            gfx::line(p + vec2(-r, 0.0), p + vec2(r, 0.0), 1.4, INK);
        }
    }
    draw_circle_lines(p.x, p.y, r, 1.3, INK);
}

fn draw_tails(g: &Geo, part: &Part, main: Color, t: f32, atk: f32) {
    let s = part.size;
    for (i, deg) in spread(part.count, -25.0, 15.0).enumerate() {
        let (p, _) = g.rim(deg);
        let wag = (t * 4.0 + i as f32).sin() * 4.0 * s + atk * 6.0;
        let pts = [
            p - vec2(4.0, 0.0),
            p + vec2(9.0 * s, -2.0 * s),
            p + vec2(16.0 * s, -8.0 * s + wag * 0.5),
            p + vec2(20.0 * s, -16.0 * s + wag),
        ];
        gfx::ribbon(
            &pts,
            &[5.0 * s + 1.8, 4.0 * s + 1.8, 3.0 * s + 1.8, 1.8],
            INK,
        );
        gfx::ribbon(
            &pts,
            &[5.0 * s, 4.0 * s, 3.0 * s, 0.6],
            gfx::shade(main, 0.9),
        );
    }
}

fn draw_wings(g: &Geo, part: &Part, accent: Color, t: f32, atk: f32) {
    let s = part.size;
    let flap = atk.max(((t * 9.0).sin() * 0.5 + 0.5) * 0.3);
    for (lo, hi) in arcs(part.side, (105.0, 130.0), (50.0, 75.0)) {
        let out = if lo > 90.0 { -1.0 } else { 1.0 };
        for deg in spread(part.count, lo, hi) {
            let (p, _) = g.rim(deg);
            let tip = p + vec2(out * (10.0 + flap * 14.0) * s, -(12.0 + flap * 14.0) * s);
            let back = p + vec2(out * 14.0 * s, -2.0 * s);
            let wing = gfx::with_alpha(gfx::mix(accent, WHITE, 0.25), 0.9);
            draw_triangle(p, tip, back, wing);
            gfx::line(p, tip, 1.4, INK);
            gfx::line(tip, back, 1.4, INK);
            gfx::line(p + (tip - p) * 0.3, back, 1.0, gfx::shade(accent, 0.7));
        }
    }
}

fn draw_fins(g: &Geo, part: &Part, accent: Color, t: f32) {
    let s = part.size;
    for (lo, hi) in arcs(part.side, (190.0, 210.0), (-10.0, 20.0)) {
        let out = if lo > 90.0 { -1.0 } else { 1.0 };
        for (i, deg) in spread(part.count, lo, hi).enumerate() {
            let (p, _) = g.rim(deg);
            let wag = (t * 6.0 + i as f32).sin() * 3.0;
            let a = p + vec2(0.0, -6.0 * s);
            let b = p + vec2(0.0, 6.0 * s);
            let tip = p + vec2(out * 13.0 * s, wag);
            draw_triangle(a, b, tip, gfx::shade(accent, 0.9));
            gfx::line(a, tip, 1.3, INK);
            gfx::line(b, tip, 1.3, INK);
        }
    }
}

fn draw_spikes(g: &Geo, part: &Part, accent: Color, atk: f32) {
    let s = part.size;
    let n = part.count * 2 + 1;
    for (lo, hi) in arcs(part.side, (100.0, 165.0), (15.0, 80.0)) {
        for deg in spread(n, lo, hi) {
            let (p, d) = g.rim(deg);
            let side = vec2(-d.y, d.x) * 3.2 * s;
            let tip = p + d * (8.0 * s + atk * 4.0);
            let base = p - d * 2.0;
            draw_triangle(base + side, base - side, tip, gfx::shade(accent, 0.85));
            gfx::line(base + side, tip, 1.1, INK);
            gfx::line(base - side, tip, 1.1, INK);
        }
    }
}

fn draw_legs(g: &Geo, ground: Vec2, part: &Part, main: Color, t: f32, fast: bool) {
    let s = part.size;
    let rate = if fast { 14.0 } else { 8.0 };
    let xs: Vec<f32> = match part.side {
        Side::Front => spread(part.count, -0.65, -0.15).collect(),
        Side::Back => spread(part.count, 0.15, 0.65).collect(),
        Side::Both => spread(part.count * 2, -0.65, 0.65).collect(),
    };
    for (i, k) in xs.into_iter().enumerate() {
        let hip = vec2(g.c.x + k * g.rx, g.c.y + g.ry * 0.8);
        let step = (t * rate + i as f32 * 2.1).sin() * 2.5;
        let foot = vec2(hip.x + k * 4.0 + step, ground.y - 1.0);
        gfx::line(hip, foot, 4.0 * s, INK);
        gfx::line(hip, foot, 2.4 * s, gfx::shade(main, 0.8));
        gfx::ellipse(foot + vec2(-1.5, 0.0), 3.5 * s, 2.0 * s, INK);
    }
}

fn draw_tentacles(g: &Geo, part: &Part, main: Color, t: f32) {
    let s = part.size;
    let xs: Vec<f32> = match part.side {
        Side::Front => spread(part.count + 1, -0.7, -0.1).collect(),
        Side::Back => spread(part.count + 1, 0.1, 0.7).collect(),
        Side::Both => spread(part.count * 2 + 1, -0.7, 0.7).collect(),
    };
    for (i, k) in xs.into_iter().enumerate() {
        let root = vec2(g.c.x + k * g.rx, g.c.y + g.ry * 0.7);
        let pts: Vec<Vec2> = (0..7)
            .map(|j| {
                let fj = j as f32;
                root + vec2((t * 3.0 + fj * 0.7 + i as f32).sin() * 2.5, fj * 3.0 * s)
            })
            .collect();
        gfx::polyline(&pts, 3.4 * s, INK);
        gfx::polyline(&pts, 2.0 * s, gfx::shade(main, 0.9));
    }
}

fn draw_claws(g: &Geo, part: &Part, accent: Color, t: f32, atk: f32) {
    let s = part.size;
    for (lo, hi) in arcs(part.side, (195.0, 215.0), (-35.0, -15.0)) {
        let out = if lo > 90.0 { -1.0 } else { 1.0 };
        for deg in spread(part.count, lo, hi) {
            let (p, _) = g.rim(deg);
            let reach = if out < 0.0 { atk * 10.0 } else { 0.0 };
            let c = p + vec2(out * (9.0 * s + reach), -4.0 * s - atk * 5.0);
            let open = 0.35 + atk * 0.9 + (t * 3.0).sin().max(0.0) * 0.15;
            gfx::line(p, c, 3.4 * s, INK);
            gfx::line(p, c, 2.0 * s, gfx::shade(accent, 0.85));
            gfx::ellipse(c, 6.5 * s, 4.8 * s, accent);
            gfx::ellipse_outline(c, 6.5 * s, 4.8 * s, 1.3, INK);
            let tip = c + vec2(out * 6.0 * s, -open * 6.0 * s);
            draw_triangle(
                c + vec2(out * 2.0, -2.0),
                tip,
                c + vec2(out * 7.5 * s, 1.0),
                accent,
            );
            gfx::line(c + vec2(out * 2.0, -2.0), tip, 1.1, INK);
        }
    }
}

fn draw_horns(g: &Geo, part: &Part, belly: Color) {
    let s = part.size;
    let horn = gfx::mix(belly, WHITE, 0.2);
    for (lo, hi) in arcs(part.side, (100.0, 130.0), (55.0, 85.0)) {
        for deg in spread(part.count, lo, hi) {
            let (p, d) = g.rim(deg);
            let side = vec2(-d.y, d.x) * 3.0 * s;
            let tip = p + d * 11.0 * s + vec2(3.0 * s, -2.0 * s);
            let base = p - d * 2.0;
            draw_triangle(base + side, base - side, tip, horn);
            gfx::line(base + side, tip, 1.3, INK);
            gfx::line(base - side, tip, 1.3, INK);
        }
    }
}

fn draw_helmet(g: &Geo, eye: f32) {
    let steel = Color::new(0.72, 0.76, 0.82, 1.0);
    let c = g.top + vec2(0.0, eye + 4.0);
    let (rx, ry) = ((g.rx * 0.55).max(12.0), 9.0);
    // The top half of an ellipse, as a fan.
    let n = 14;
    let pt = |i: usize| {
        let a = PI + i as f32 / n as f32 * PI;
        c + vec2(a.cos() * rx, a.sin() * ry)
    };
    for i in 0..n {
        draw_triangle(c, pt(i), pt(i + 1), steel);
    }
    let rim: Vec<Vec2> = (0..=n).map(pt).collect();
    gfx::polyline(&rim, 1.6, INK);
    draw_rectangle(
        c.x - rx - 1.0,
        c.y - 2.0,
        rx * 2.0 + 2.0,
        4.0,
        gfx::shade(steel, 0.85),
    );
    draw_rectangle_lines(c.x - rx - 1.0, c.y - 2.0, rx * 2.0 + 2.0, 4.0, 1.3, INK);
    draw_circle(c.x - rx * 0.4, c.y - ry * 0.5, 1.6, WHITE);
}

fn draw_crown(top: Vec2) {
    let gold = Color::new(1.0, 0.82, 0.25, 1.0);
    let b = top + vec2(0.0, -1.0);
    let (w, h) = (16.0, 5.0);
    let pts = [
        vec2(b.x - w * 0.5, b.y),
        vec2(b.x - w * 0.5, b.y - h - 5.0),
        vec2(b.x - w * 0.25, b.y - h),
        vec2(b.x, b.y - h - 7.0),
        vec2(b.x + w * 0.25, b.y - h),
        vec2(b.x + w * 0.5, b.y - h - 5.0),
        vec2(b.x + w * 0.5, b.y),
    ];
    draw_rectangle(b.x - w * 0.5, b.y - h, w, h, gold);
    for i in [1usize, 3, 5] {
        draw_triangle(
            pts[i - 1].with_y(b.y - h),
            pts[i],
            pts[i + 1].with_y(b.y - h),
            gold,
        );
    }
    let mut outline = pts.to_vec();
    outline.push(pts[0]);
    gfx::polyline(&outline, 1.4, INK);
    draw_circle(b.x, b.y - h * 0.5, 1.6, Color::new(0.85, 0.25, 0.35, 1.0));
}

fn draw_bow_tie(at: Vec2, accent: Color) {
    let c = gfx::shade(accent, 0.9);
    for side in [-1.0f32, 1.0] {
        let a = at + vec2(side * 8.0, -4.5);
        let b = at + vec2(side * 8.0, 4.5);
        draw_triangle(at, a, b, c);
        gfx::line(at, a, 1.2, INK);
        gfx::line(at, b, 1.2, INK);
        gfx::line(a, b, 1.2, INK);
    }
    draw_circle(at.x, at.y, 2.4, c);
    draw_circle_lines(at.x, at.y, 2.4, 1.2, INK);
}

fn draw_leaf(top: Vec2, t: f32) {
    let green = Color::new(0.45, 0.76, 0.36, 1.0);
    let sway = (t * 1.8).sin() * 12.0;
    let stem_top = top + vec2(1.0, -6.0);
    gfx::line(top + vec2(0.0, 1.0), stem_top, 1.6, INK);
    let leaf = stem_top + vec2(6.0, -3.0);
    gfx::ellipse_rot(leaf, 8.0, 3.6, -25.0 + sway, gfx::shade(green, 0.6));
    gfx::ellipse_rot(leaf, 7.0, 2.8, -25.0 + sway, green);
    gfx::line(
        stem_top,
        leaf + vec2(5.0, -2.0),
        1.0,
        gfx::shade(green, 0.6),
    );
}

#[cfg(test)]
mod tests {
    use crate::content::monster_spec::INK_RGB;

    #[test]
    fn the_palette_check_uses_the_real_ink() {
        let ink = super::gfx::INK;
        assert_eq!([ink.r, ink.g, ink.b], INK_RGB);
    }

    #[test]
    fn spread_covers_the_range() {
        let v: Vec<f32> = super::spread(3, 0.0, 10.0).collect();
        assert_eq!(v, [0.0, 5.0, 10.0]);
        assert_eq!(super::spread(1, 0.0, 10.0).collect::<Vec<_>>(), [5.0]);
    }
}
