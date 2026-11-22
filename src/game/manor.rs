//! The gothic manor across the lake (FR24): the mad scientist's home, drawn
//! as a backdrop rather than placed in the world. The road view pins it over
//! the lake with a slow drift (`World::draw_manor`), and the battle view puts
//! it on the horizon (`battle::draw_backdrop`). Both go through `draw`.
//!
//! The building is described once in local units (x right, y up negative,
//! the island's waterline at y = 0) and mapped through a closure, so the
//! same shapes also draw its rippling reflection in the lake.

use super::gfx::{self, INK, LAB_GREEN, LANTERN, MOON};
use macroquad::prelude::*;

/// Seconds between lightning strikes, before jitter.
const STRIKE_EVERY_S: f32 = 9.0;
/// How long one strike's flash lasts.
const STRIKE_S: f32 = 0.55;

const STONE: Color = Color::new(0.13, 0.13, 0.20, 1.0);
const STONE_DARK: Color = Color::new(0.09, 0.09, 0.15, 1.0);
const ROOF: Color = Color::new(0.10, 0.08, 0.16, 1.0);
const ISLAND: Color = Color::new(0.06, 0.07, 0.09, 1.0);

/// Brightness of the lightning at game time `t`: 0 most of the time, a
/// double flicker up to 1 during a strike. Strike times come from
/// `gfx::hash01` on the strike index, so they're the same every run and
/// freeze with the game clock.
pub fn lightning(t: f32) -> f32 {
    let n = (t / STRIKE_EVERY_S).floor();
    let at = n * STRIKE_EVERY_S + gfx::hash01(n as i64 as u64 ^ 0x11_6847) * (STRIKE_EVERY_S - 2.0);
    let u = (t - at) / STRIKE_S;
    if !(0.0..1.0).contains(&u) {
        return 0.0;
    }
    // Bright, dip, a second smaller flash, then fade.
    let a = (1.0 - u / 0.3).clamp(0.0, 1.0);
    let b = if u > 0.4 {
        0.7 * (1.0 - (u - 0.4) / 0.6)
    } else {
        0.0
    };
    a.max(b)
}

/// Which strike `t` falls in, so a bolt keeps its shape for the whole flash.
fn strike_seed(t: f32) -> u64 {
    (t / STRIKE_EVERY_S).floor() as i64 as u64
}

/// Draws the manor with its island's waterline centre at `at`, `scale` 1
/// being about 230 px wide and 180 px tall. `reflection` adds its mirror in
/// the water below (the road view); the battle view leaves it off. Under
/// `reduce_flashing` there's no strike, only a steady pale glow behind the
/// towers.
pub fn draw(at: Vec2, scale: f32, t: f32, reflection: bool, reduce_flashing: bool) {
    draw_fading(at, scale, t, reflection, reduce_flashing, 1.0);
}

/// [`draw`] seen through a haze: `fade` runs from 1 (as drawn) to 0 (gone), so
/// the manor can melt away as the castle, its near self, comes into view
/// (FR27). Its bolt and bats only show while it's mostly there.
pub fn draw_fading(
    at: Vec2,
    scale: f32,
    t: f32,
    reflection: bool,
    reduce_flashing: bool,
    fade: f32,
) {
    if fade <= 0.0 {
        return;
    }
    let flash = if reduce_flashing { 0.0 } else { lightning(t) };

    // The sky lit up behind the towers: a steady glow, brighter in a strike.
    let behind = at + vec2(10.0, -120.0) * scale;
    // The road view (with a reflection) is over dark water, so its glow is
    // fainter than the battle view's sky.
    let base = if reflection { 0.04 } else { 0.08 };
    let steady = if reduce_flashing { base * 2.0 } else { base };
    gfx::glow(
        behind,
        150.0 * scale,
        110.0 * scale,
        gfx::with_alpha(MOON, (steady + 0.35 * flash) * fade),
    );
    if flash > 0.15 && fade > 0.5 {
        draw_bolt(at, scale, strike_seed(t), flash);
    }

    if reflection {
        let m = |p: Vec2| {
            let y = -p.y * 0.55;
            at + vec2(p.x + 2.5 * (t * 1.6 + y * 0.25).sin(), y) * scale
        };
        building(&m, 0.16 * fade, t, flash, false, true);
    }
    let m = |p: Vec2| at + p * scale;
    building(&m, fade, t, flash, true, true);

    if fade > 0.6 {
        bats_round_the_tower(at, scale, t);
    }
}

/// The castle's keep (FR27): the manor seen from close, with no island under
/// it. `at` is where the foot of its walls stands. It's the same building as
/// the one across the lake, with the same glow, storms and bats.
pub fn draw_keep(at: Vec2, scale: f32, t: f32, reduce_flashing: bool) {
    let flash = if reduce_flashing { 0.0 } else { lightning(t) };
    let behind = at + vec2(10.0, -120.0) * scale;
    let steady = if reduce_flashing { 0.16 } else { 0.08 };
    gfx::glow(
        behind,
        170.0 * scale,
        130.0 * scale,
        gfx::with_alpha(MOON, steady + 0.35 * flash),
    );
    if flash > 0.15 {
        draw_bolt(at, scale, strike_seed(t), flash);
    }
    let m = |p: Vec2| at + p * scale;
    building(&m, 1.0, t, flash, true, false);
    bats_round_the_tower(at, scale, t);
}

/// Bats circling the tall tower.
fn bats_round_the_tower(at: Vec2, scale: f32, t: f32) {
    let top = at + vec2(0.0, -150.0) * scale;
    for i in 0..4 {
        let fi = i as f32;
        let a = t * (0.7 + fi * 0.13) + fi * 1.7;
        let p = top + vec2(a.cos() * (46.0 + fi * 8.0), a.sin() * 16.0 - fi * 6.0) * scale;
        bat(p, scale * (0.8 + 0.1 * fi), (t * 9.0 + fi * 2.0).sin());
    }
}

/// A little bat silhouette at `c`; `flap` in -1..1.
pub fn bat(c: Vec2, scale: f32, flap: f32) {
    let col = Color::new(0.05, 0.04, 0.08, 1.0);
    let w = 9.0 * scale;
    let lift = flap * 4.0 * scale;
    for side in [-1.0f32, 1.0] {
        let tip = c + vec2(side * w, -2.0 * scale - lift);
        let mid = c + vec2(side * w * 0.5, 1.5 * scale - lift * 0.4);
        draw_triangle(c + vec2(0.0, -1.5 * scale), tip, mid, col);
        draw_triangle(
            c + vec2(0.0, 1.5 * scale),
            mid,
            tip + vec2(-side * 2.0 * scale, 3.0 * scale),
            col,
        );
    }
    draw_circle(c.x, c.y, 2.4 * scale, col);
    draw_triangle(
        c + vec2(-2.0, -1.5) * scale,
        c + vec2(-1.0, -4.5) * scale,
        c + vec2(0.0, -1.5) * scale,
        col,
    );
    draw_triangle(
        c + vec2(2.0, -1.5) * scale,
        c + vec2(1.0, -4.5) * scale,
        c + vec2(0.0, -1.5) * scale,
        col,
    );
}

fn draw_bolt(at: Vec2, scale: f32, seed: u64, flash: f32) {
    let rod = at + vec2(0.0, -176.0) * scale;
    let sky = rod + vec2(-60.0 + 120.0 * gfx::hash01(seed ^ 0xB0), -150.0) * scale;
    let mut pts = vec![sky];
    for i in 1..7 {
        let u = i as f32 / 7.0;
        let j = (gfx::hash01(seed.wrapping_mul(31) + i) - 0.5) * 34.0 * scale;
        pts.push(sky.lerp(rod, u) + vec2(j, 0.0));
    }
    pts.push(rod);
    gfx::polyline(&pts, 5.0 * scale, gfx::with_alpha(MOON, 0.35 * flash));
    gfx::polyline(&pts, 2.0 * scale, gfx::with_alpha(WHITE, flash));
}

/// The building itself, through the mapping `m`. `alpha` fades it (the
/// reflection); `rim` adds the moonlit edges, which a reflection skips;
/// `island` draws the rock it stands on (the castle's keep stands on a wall).
fn building(m: &dyn Fn(Vec2) -> Vec2, alpha: f32, t: f32, flash: f32, rim: bool, island: bool) {
    let a = |c: Color| gfx::with_alpha(c, c.a * alpha);
    let lit = |c: Color| a(gfx::mix(c, MOON, 0.35 * flash));
    let quad = |x0: f32, y0: f32, x1: f32, y1: f32, c: Color| {
        gfx::quad(
            m(vec2(x0, y0)),
            m(vec2(x1, y0)),
            m(vec2(x1, y1)),
            m(vec2(x0, y1)),
            c,
        );
    };
    let tri = |p: [(f32, f32); 3], c: Color| {
        draw_triangle(
            m(vec2(p[0].0, p[0].1)),
            m(vec2(p[1].0, p[1].1)),
            m(vec2(p[2].0, p[2].1)),
            c,
        );
    };
    // Walls: (x0, x1, top, roof peak). Wings, the hall, the turrets.
    let walls: [(f32, f32, f32, f32); 6] = [
        (-96.0, -50.0, -58.0, -86.0),
        (48.0, 92.0, -52.0, -78.0),
        (-52.0, 50.0, -76.0, -108.0),
        (-86.0, -70.0, -104.0, -140.0),
        (74.0, 88.0, -88.0, -116.0),
        (-13.0, 13.0, -132.0, -176.0),
    ];

    // The island: a dark rock shelf with a moonlit top edge.
    let shore = [
        (-132.0, 0.0),
        (-118.0, -12.0),
        (-96.0, -16.0),
        (-60.0, -20.0),
        (-20.0, -24.0),
        (50.0, -22.0),
        (96.0, -16.0),
        (118.0, -10.0),
        (130.0, 0.0),
    ];
    for w in shore.windows(2).filter(|_| island) {
        gfx::quad(
            m(vec2(w[0].0, w[0].1)),
            m(vec2(w[1].0, w[1].1)),
            m(vec2(w[1].0, 0.0)),
            m(vec2(w[0].0, 0.0)),
            lit(ISLAND),
        );
        if rim {
            gfx::line(
                m(vec2(w[0].0, w[0].1)),
                m(vec2(w[1].0, w[1].1)),
                1.2,
                a(gfx::with_alpha(MOON, 0.25)),
            );
        }
    }
    // Rocks tumbling into the water along the front.
    for (x, r) in [
        (-112.0, 7.0),
        (-70.0, 9.0),
        (-30.0, 6.0),
        (24.0, 8.0),
        (70.0, 10.0),
        (108.0, 6.0),
    ]
    .into_iter()
    .filter(|_| island)
    {
        let c = m(vec2(x, -2.0));
        let rr = (m(vec2(r, 0.0)) - m(vec2(0.0, 0.0))).length();
        gfx::ellipse(c, rr, rr * 0.55, lit(gfx::shade(ISLAND, 1.5)));
    }
    // Crooked chimney behind the hall roof.
    gfx::quad(
        m(vec2(26.0, -96.0)),
        m(vec2(34.0, -96.0)),
        m(vec2(38.0, -124.0)),
        m(vec2(29.0, -125.0)),
        lit(STONE_DARK),
    );
    // Walls and roofs, back to front: the tall tower last.
    for (i, &(x0, x1, top, peak)) in walls.iter().enumerate() {
        let base = if i == 5 { -100.0 } else { -14.0 };
        quad(
            x0,
            base,
            x1,
            top,
            lit(if i % 2 == 0 { STONE } else { STONE_DARK }),
        );
        let over = (x1 - x0) * 0.12;
        tri(
            [(x0 - over, top), (x1 + over, top), ((x0 + x1) * 0.5, peak)],
            lit(ROOF),
        );
        if rim {
            gfx::line(
                m(vec2(x0, base)),
                m(vec2(x0, top)),
                1.3,
                a(gfx::with_alpha(MOON, 0.35)),
            );
            gfx::line(
                m(vec2(x0 - over, top)),
                m(vec2((x0 + x1) * 0.5, peak)),
                1.3,
                a(gfx::with_alpha(MOON, 0.40)),
            );
        }
        // Battlement teeth on the wings.
        if i < 2 {
            let mut x = x0 + 2.0;
            while x + 5.0 < x1 {
                quad(x, top, x + 5.0, top - 5.0, lit(STONE));
                x += 9.0;
            }
        }
    }
    // The tower's lightning rod.
    gfx::line(m(vec2(0.0, -176.0)), m(vec2(0.0, -190.0)), 1.6, a(INK));

    // Windows: arched, most warm and lit, a few dark. The flicker is slow
    // and small, a candle rather than a blink.
    let windows: [(f32, f32, bool); 12] = [
        (-86.0, -32.0, true),
        (-66.0, -32.0, false),
        (-38.0, -38.0, true),
        (-20.0, -38.0, true),
        (20.0, -38.0, false),
        (38.0, -38.0, true),
        (-30.0, -60.0, true),
        (30.0, -60.0, true),
        (62.0, -30.0, true),
        (80.0, -30.0, false),
        (-78.0, -84.0, true),
        (81.0, -72.0, true),
    ];
    for (i, &(x, y, on)) in windows.iter().enumerate() {
        let candle = 0.85 + 0.15 * (t * 1.3 + i as f32 * 1.9).sin();
        let col = if on {
            gfx::with_alpha(gfx::shade(LANTERN, candle), 1.0)
        } else {
            Color::new(0.05, 0.05, 0.09, 1.0)
        };
        quad(x - 3.0, y, x + 3.0, y - 9.0, a(col));
        tri(
            [(x - 3.0, y - 9.0), (x + 3.0, y - 9.0), (x, y - 13.0)],
            a(col),
        );
    }
    // The lab: a round green window high in the tower.
    let lab = m(vec2(0.0, -150.0));
    let pulse = 0.8 + 0.2 * (t * 0.9).sin();
    let r = (m(vec2(6.0, 0.0)) - m(vec2(0.0, 0.0))).length();
    if rim {
        gfx::glow(
            lab,
            r * 3.5,
            r * 3.5,
            gfx::with_alpha(LAB_GREEN, 0.30 * pulse * alpha),
        );
    }
    draw_circle(lab.x, lab.y, r, a(gfx::shade(LAB_GREEN, pulse)));
    gfx::line(lab - vec2(r, 0.0), lab + vec2(r, 0.0), 1.0, a(STONE_DARK));
    gfx::line(lab - vec2(0.0, r), lab + vec2(0.0, r), 1.0, a(STONE_DARK));
    // The great door.
    quad(
        -7.0,
        -14.0,
        7.0,
        -28.0,
        a(Color::new(0.20, 0.12, 0.10, 1.0)),
    );
    tri(
        [(-7.0, -28.0), (7.0, -28.0), (0.0, -34.0)],
        a(Color::new(0.20, 0.12, 0.10, 1.0)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lightning_strikes_now_and_then_and_is_repeatable() {
        let samples: Vec<f32> = (0..60 * 90).map(|i| lightning(i as f32 / 60.0)).collect();
        let lit = samples.iter().filter(|&&b| b > 0.0).count();
        // Some frames flash, most don't: about one 0.55 s strike per 9 s.
        assert!(lit > 0, "never strikes");
        assert!(
            (lit as f32) < samples.len() as f32 * 0.12,
            "{lit} lit frames"
        );
        assert!(samples.iter().all(|b| (0.0..=1.0).contains(b)));
        let again: Vec<f32> = (0..60 * 90).map(|i| lightning(i as f32 / 60.0)).collect();
        assert_eq!(samples, again);
    }
}
