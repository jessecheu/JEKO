//! The castle at the end of the road (FR27, milestone C): a curtain wall across
//! the whole screen with a gatehouse in the road's line, and behind it the
//! keep, which is the manor across the lake seen up close (the owner's answer:
//! one home for the mad scientist).
//!
//! It is scenery with one state, `open`: how far the portcullis has risen, for
//! the arrival (0 = shut, 1 = open). Nothing here changes a rule (invariant 7).
//! The wall's front stands at `world::CASTLE_WALL_Y`; everything north of it
//! is the castle's, so the world places and draws nothing past it.

use super::gfx::{self, INK, LAB_GREEN, LANTERN, MOON, SHADOW};
use super::manor;
use super::world::{self, View};
use macroquad::prelude::*;

/// Height of the curtain wall's face above its foot.
const WALL_H: f32 = 170.0;
/// Half the width of the gate's opening: a little narrower than the road.
const GATE_HALF: f32 = 62.0;
/// Height of the gate's straight sides; the arch rises another `ARCH_H`.
const GATE_H: f32 = 100.0;
const ARCH_H: f32 = 26.0;
const TOWER_W: f32 = 66.0;
const TOWER_H: f32 = 224.0;
/// Scale of the keep behind the wall: 1 is the manor's size across the lake.
const KEEP_SCALE: f32 = 1.05;

const STONE: Color = Color::new(0.17, 0.17, 0.24, 1.0);
const STONE_LIT: Color = Color::new(0.25, 0.25, 0.34, 1.0);
const STONE_DARK: Color = Color::new(0.11, 0.11, 0.17, 1.0);
const ROOF: Color = Color::new(0.10, 0.08, 0.16, 1.0);
const IRON: Color = Color::new(0.09, 0.09, 0.13, 1.0);
const MORTAR: Color = Color::new(0.06, 0.06, 0.11, 0.5);
const CARVED: Color = Color::new(0.80, 0.78, 0.70, 1.0);

/// Draws the castle if any of it is on screen. Painted after the road and the
/// lights and before the depth-sorted pass: it covers everything north of its
/// foot, and nothing stands north of that.
pub fn draw(view: &View, t: f32, open: f32, reduce_flashing: bool) {
    let foot = view.to_screen(vec2(
        world::path_center(world::CASTLE_GATE_Y),
        world::CASTLE_WALL_Y,
    ));
    let (cx, by) = (foot.x, foot.y);
    if by <= 0.0 {
        return;
    }
    let sw = screen_width();
    let top = by - WALL_H;

    // The night behind the wall, with the keep standing up out of it.
    if top > 0.0 {
        draw_rectangle(0.0, 0.0, sw, top + 2.0, Color::new(0.03, 0.03, 0.065, 1.0));
        for i in 0..36u64 {
            let x = gfx::hash01(i * 7 + 1) * sw;
            let y = gfx::hash01(i * 7 + 2) * (top - 20.0).max(0.0);
            let twinkle = if reduce_flashing {
                0.7
            } else {
                0.55 + 0.45 * (t * 1.3 + i as f32).sin()
            };
            draw_circle(x, y, 0.9, gfx::with_alpha(MOON, 0.5 * twinkle));
        }
        manor::draw_keep(
            vec2(cx, top + KEEP_SCALE * 18.0),
            KEEP_SCALE,
            t,
            reduce_flashing,
        );
    }

    wall(cx, by, sw);
    for side in [-1.0f32, 1.0] {
        tower(
            cx + side * (GATE_HALF + TOWER_W * 0.5),
            by,
            side,
            t,
            reduce_flashing,
        );
    }
    gate(cx, by, open, t);
    plaque(cx, by);

    // The wall's shadow on the cobbles in front of it.
    draw_rectangle(0.0, by, sw, 12.0, SHADOW);
    draw_rectangle(0.0, by + 12.0, sw, 10.0, gfx::with_alpha(SHADOW, 0.16));
}

/// The curtain wall: a brick face with a moonlit rim and a row of teeth.
fn wall(cx: f32, by: f32, sw: f32) {
    let top = by - WALL_H;
    draw_rectangle(0.0, top, sw, WALL_H, STONE);
    // Darker toward the foot, as if the lanterns don't reach it.
    draw_rectangle(
        0.0,
        top + WALL_H * 0.55,
        sw,
        WALL_H * 0.45,
        gfx::with_alpha(STONE_DARK, 0.5),
    );
    bricks(0.0, sw, top, by, 0.0);
    draw_rectangle(0.0, top, sw, 2.0, gfx::with_alpha(MOON, 0.28));
    // Battlements, anchored to the gate so they don't swim when the camera moves.
    let step = 28.0;
    let mut x = cx.rem_euclid(step) - step;
    while x < sw {
        draw_rectangle(x, top - 12.0, 16.0, 12.0, STONE_LIT);
        draw_rectangle(x, top - 12.0, 16.0, 2.0, gfx::with_alpha(MOON, 0.3));
        x += step;
    }
}

/// Mortar lines over `[x0, x1] x [y0, y1]`: rows 15 px high, joints staggered.
fn bricks(x0: f32, x1: f32, y0: f32, y1: f32, phase: f32) {
    let (row_h, brick_w) = (15.0, 34.0);
    let mut y = y0 + row_h;
    let mut row = 0;
    while y < y1 {
        gfx::line(vec2(x0, y), vec2(x1, y), 1.0, MORTAR);
        let mut x =
            x0 + ((phase + if row % 2 == 0 { 0.0 } else { brick_w * 0.5 }).rem_euclid(brick_w));
        while x < x1 {
            gfx::line(vec2(x, y - row_h), vec2(x, y), 1.0, MORTAR);
            x += brick_w;
        }
        y += row_h;
        row += 1;
    }
}

/// A gatehouse tower on `x` (its middle), with a pointed roof, arrow slits that
/// glow the lab's green, and a banner bearing a zero.
fn tower(x: f32, by: f32, side: f32, t: f32, reduce_flashing: bool) {
    let (l, r) = (x - TOWER_W * 0.5, x + TOWER_W * 0.5);
    let top = by - TOWER_H;
    draw_rectangle(l, top, TOWER_W, TOWER_H, STONE_LIT);
    draw_rectangle(
        x + side * TOWER_W * 0.05,
        top,
        TOWER_W * 0.5,
        TOWER_H,
        gfx::with_alpha(STONE_DARK, 0.28),
    );
    bricks(l, r, top, by, side * 9.0);
    draw_line(l, top, l, by, 1.4, gfx::with_alpha(MOON, 0.35));
    // A ledge and the teeth under the roof.
    draw_rectangle(l - 5.0, top, TOWER_W + 10.0, 8.0, STONE_LIT);
    let mut tx = l - 5.0;
    while tx + 9.0 <= r + 5.0 {
        draw_rectangle(tx, top - 9.0, 9.0, 9.0, STONE_LIT);
        tx += 16.0;
    }
    // The roof.
    let roof_h = 52.0;
    draw_triangle(
        vec2(l - 9.0, top - 9.0),
        vec2(r + 9.0, top - 9.0),
        vec2(x, top - 9.0 - roof_h),
        ROOF,
    );
    draw_line(
        l - 9.0,
        top - 9.0,
        x,
        top - 9.0 - roof_h,
        1.4,
        gfx::with_alpha(MOON, 0.4),
    );
    draw_line(
        x,
        top - 9.0 - roof_h,
        x,
        top - 9.0 - roof_h - 14.0,
        1.6,
        INK,
    );
    // Arrow slits, lit from inside.
    for (i, dy) in [46.0f32, 100.0].into_iter().enumerate() {
        let pulse = if reduce_flashing {
            0.85
        } else {
            0.8 + 0.2 * (t * 1.1 + i as f32 * 1.9 + side).sin()
        };
        let c = vec2(x, top + dy);
        gfx::glow(c, 18.0, 22.0, gfx::with_alpha(LAB_GREEN, 0.18 * pulse));
        draw_rectangle(
            c.x - 2.5,
            c.y - 10.0,
            5.0,
            20.0,
            gfx::shade(LAB_GREEN, pulse),
        );
    }
    // The banner: the doctor's zero, ringed in brass.
    let b = vec2(x, top + 150.0);
    let sway = if reduce_flashing {
        0.0
    } else {
        (t * 1.4 + side).sin() * 1.5
    };
    draw_rectangle(b.x - 13.0, b.y - 18.0, 26.0, 3.0, IRON);
    gfx::quad(
        vec2(b.x - 12.0, b.y - 15.0),
        vec2(b.x + 12.0, b.y - 15.0),
        vec2(b.x + 12.0 + sway, b.y + 26.0),
        vec2(b.x + sway, b.y + 34.0),
        Color::new(0.20, 0.30, 0.20, 1.0),
    );
    gfx::quad(
        vec2(b.x - 12.0, b.y - 15.0),
        vec2(b.x + sway, b.y + 34.0),
        vec2(b.x - 12.0 + sway, b.y + 26.0),
        vec2(b.x - 12.0, b.y - 15.0),
        Color::new(0.14, 0.22, 0.15, 1.0),
    );
    gfx::ellipse_outline(
        vec2(b.x + sway * 0.4, b.y + 6.0),
        6.0,
        8.0,
        2.0,
        Color::new(0.78, 0.60, 0.24, 1.0),
    );
}

/// The gateway: a dark arch with the lab's glow inside, a portcullis that
/// rises with `open`, and a torch on each side.
fn gate(cx: f32, by: f32, open: f32, t: f32) {
    let (l, r) = (cx - GATE_HALF, cx + GATE_HALF);
    let spring = by - GATE_H;
    let dark = Color::new(0.012, 0.018, 0.035, 1.0);
    // The opening and its arch.
    draw_rectangle(l, spring, GATE_HALF * 2.0, GATE_H, dark);
    let arch: Vec<Vec2> = (0..=24)
        .map(|i| {
            let a = std::f32::consts::PI * i as f32 / 24.0;
            vec2(cx - a.cos() * GATE_HALF, spring - a.sin() * ARCH_H)
        })
        .collect();
    for w in arch.windows(2) {
        draw_triangle(vec2(cx, spring), w[0], w[1], dark);
    }
    // What's inside: the lab's glow, stronger as the gate rises.
    let spill = 0.08 + 0.34 * open;
    gfx::glow(
        vec2(cx, by - 34.0),
        GATE_HALF * 1.2,
        GATE_H * 0.7,
        gfx::with_alpha(LAB_GREEN, spill),
    );
    // The stone frame.
    gfx::polyline(&arch, 7.0, STONE_LIT);
    gfx::polyline(&arch, 1.6, gfx::with_alpha(MOON, 0.35));
    draw_rectangle(l - 6.0, spring, 6.0, GATE_H, STONE_LIT);
    draw_rectangle(r, spring, 6.0, GATE_H, STONE_LIT);

    // The portcullis hangs from the arch and rises into it.
    let rise = open.clamp(0.0, 1.0) * (GATE_H + ARCH_H + 6.0);
    let bottom = by - rise;
    let ceiling = spring - ARCH_H + 4.0;
    if bottom > ceiling {
        let n = 9;
        for i in 0..n {
            let x = l + 8.0 + i as f32 * (GATE_HALF * 2.0 - 16.0) / (n - 1) as f32;
            // Inside the arch's curve at this x.
            let u = ((x - cx) / GATE_HALF).clamp(-1.0, 1.0);
            let arch_top = spring - ARCH_H * (1.0 - u * u).sqrt();
            let y0 = arch_top.max(ceiling);
            if bottom > y0 {
                draw_rectangle(x - 1.8, y0, 3.6, bottom - y0, IRON);
                draw_line(
                    x - 0.8,
                    y0,
                    x - 0.8,
                    bottom,
                    1.0,
                    gfx::with_alpha(MOON, 0.18),
                );
                draw_triangle(
                    vec2(x - 3.0, bottom),
                    vec2(x + 3.0, bottom),
                    vec2(x, bottom + 7.0),
                    IRON,
                );
            }
        }
        for dy in [18.0f32, 54.0, 90.0] {
            let y = bottom - dy;
            if y > spring - ARCH_H * 0.4 {
                draw_rectangle(l, y - 1.6, GATE_HALF * 2.0, 3.2, IRON);
            }
        }
    }

    // A torch on each side of the arch.
    for side in [-1.0f32, 1.0] {
        let p = vec2(cx + side * (GATE_HALF + 15.0), by - 66.0);
        draw_rectangle(p.x - 2.0, p.y, 4.0, 14.0, IRON);
        draw_rectangle(p.x - 4.0, p.y - 3.0, 8.0, 5.0, IRON);
        let flick = 0.85 + 0.15 * (t * 9.0 + side * 2.0).sin() * (t * 5.3).sin();
        gfx::glow(
            p - vec2(0.0, 8.0),
            54.0 * flick,
            46.0 * flick,
            gfx::with_alpha(LANTERN, 0.26),
        );
        draw_circle(p.x, p.y - 8.0, 4.6 * flick, gfx::shade(LANTERN, 0.95));
        draw_triangle(
            vec2(p.x - 3.4, p.y - 8.0),
            vec2(p.x + 3.4, p.y - 8.0),
            vec2(p.x, p.y - 8.0 - 11.0 * flick),
            LANTERN,
        );
        draw_circle(p.x, p.y - 7.0, 2.0, Color::new(1.0, 0.95, 0.7, 1.0));
    }
}

/// The carved board over the gate.
fn plaque(cx: f32, by: f32) {
    let y = by - GATE_H - ARCH_H - 20.0;
    let (w, h) = (126.0, 20.0);
    draw_rectangle(cx - w * 0.5, y - h * 0.5, w, h, STONE_DARK);
    draw_rectangle_lines(
        cx - w * 0.5,
        y - h * 0.5,
        w,
        h,
        1.5,
        gfx::with_alpha(MOON, 0.3),
    );
    let text = "THE CASTLE";
    let size = 17;
    let dim = measure_text(text, None, size, 1.0);
    draw_text(
        text,
        cx - dim.width * 0.5,
        y + dim.height * 0.4,
        size as f32,
        CARVED,
    );
}

// The shapes have to agree with each other and with the road; checked when it compiles.
const _: () = {
    // The player walks `PATH_HALF_W - WALK_MARGIN` either side of the middle, and the opening has to take them.
    assert!(GATE_HALF >= world::PATH_HALF_W - world::WALK_MARGIN);
    assert!(GATE_HALF < world::PATH_HALF_W);
    // The arch and the plaque over it fit inside the wall, and the towers stand above it.
    assert!(GATE_H + ARCH_H + 24.0 < WALL_H + 4.0);
    assert!(TOWER_H > WALL_H);
};
