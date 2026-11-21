//! Small drawing vocabulary shared by the forest world, its decorations,
//! the battle view and the player. Everything in the scene is flat-colored
//! primitives, so these helpers are what keep the "storybook cartoon" look
//! consistent: one ink color for outlines, one cool night shadow, the same
//! shading math for every lit/unlit face, and the gradient light pools that
//! make the night readable (FR24).

use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;

/// Deep plum ink instead of pure black: reads as a cartoon outline without
/// the harsher, more technical look of true black, and sits well on the
/// night palette.
pub const INK: Color = Color::new(0.13, 0.07, 0.12, 0.88);

/// Cool, moonlit ground shadow. Darker than a daytime shadow so props still
/// read as standing on the dark forest floor.
pub const SHADOW: Color = Color::new(0.02, 0.02, 0.07, 0.34);

/// Moonlight: the cool highlight on stone, water and rims.
pub const MOON: Color = Color::new(0.78, 0.84, 0.96, 1.0);
/// Lantern and candle light: the warm light pools along the road.
pub const LANTERN: Color = Color::new(1.0, 0.70, 0.28, 1.0);
/// The mad scientist's glow (FR23's lens colour): potion flasks, rune
/// stones, cauldrons, anything the doctor made.
pub const LAB_GREEN: Color = Color::new(0.49, 1.0, 0.42, 1.0);
/// The night itself: the clear colour and the vignette's tint.
pub const NIGHT: Color = Color::new(0.03, 0.04, 0.08, 1.0);

pub fn shade(c: Color, f: f32) -> Color {
    Color::new(
        (c.r * f).min(1.0),
        (c.g * f).min(1.0),
        (c.b * f).min(1.0),
        c.a,
    )
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

pub fn quad(a: Vec2, b: Vec2, c: Vec2, d: Vec2, color: Color) {
    draw_triangle(a, b, c, color);
    draw_triangle(a, c, d, color);
}

pub fn line(a: Vec2, b: Vec2, thickness: f32, color: Color) {
    draw_line(a.x, a.y, b.x, b.y, thickness, color);
}

pub fn polyline(points: &[Vec2], thickness: f32, color: Color) {
    for w in points.windows(2) {
        line(w[0], w[1], thickness, color);
    }
}

pub fn ellipse(center: Vec2, rx: f32, ry: f32, color: Color) {
    draw_ellipse(center.x, center.y, rx, ry, 0.0, color);
}

pub fn ellipse_rot(center: Vec2, rx: f32, ry: f32, degrees: f32, color: Color) {
    draw_ellipse(center.x, center.y, rx, ry, degrees, color);
}

pub fn ellipse_outline(center: Vec2, rx: f32, ry: f32, thickness: f32, color: Color) {
    draw_ellipse_lines(center.x, center.y, rx, ry, 0.0, thickness, color);
}

pub fn ground_shadow(center: Vec2, rx: f32, ry: f32) {
    ellipse(center, rx, ry, SHADOW);
}

/// Axis-free rectangle: `half` extents rotated by `angle` radians about
/// `center`. Used for leaves, planks and gravestones that shouldn't all sit square to
/// the screen.
pub fn rot_rect(center: Vec2, half: Vec2, angle: f32, color: Color) {
    let (s, c) = angle.sin_cos();
    let ax = vec2(c, s) * half.x;
    let ay = vec2(-s, c) * half.y;
    quad(
        center - ax - ay,
        center + ax - ay,
        center + ax + ay,
        center - ax + ay,
        color,
    );
}

/// Filled n-pointed star as a triangle fan: stars, glints, sparkles.
/// `squash` < 1 flattens it vertically so something lying on the ground
/// matches the foreshortening of the elliptical ground shadows.
pub fn star(
    center: Vec2,
    outer: f32,
    inner: f32,
    points: usize,
    rot: f32,
    squash: f32,
    color: Color,
) {
    let n = points * 2;
    let vertex = |i: usize| {
        let r = if i.is_multiple_of(2) { outer } else { inner };
        let a = rot + i as f32 / n as f32 * std::f32::consts::TAU;
        center + vec2(a.cos(), a.sin() * squash) * r
    };
    for i in 0..n {
        draw_triangle(center, vertex(i), vertex(i + 1), color);
    }
}

/// A strip that follows `points`, `widths[i]` wide (half-width each side)
/// at each point: the building block for branches, fern fronds and grass,
/// which need to taper and curve rather than be straight lines.
pub fn ribbon(points: &[Vec2], widths: &[f32], color: Color) {
    for i in 0..points.len().saturating_sub(1) {
        let (a, b) = (points[i], points[i + 1]);
        let n = (b - a).perp().normalize_or_zero();
        quad(
            a + n * widths[i],
            b + n * widths[i + 1],
            b - n * widths[i + 1],
            a - n * widths[i],
            color,
        );
    }
}

/// A filled elliptical ring whose colour runs from `c0` on the inner
/// ellipse to `c1` on the outer one, as one vertex-coloured mesh. With a
/// zero inner radius it's a soft disc: the building block for light pools
/// and the darkness vignette.
pub fn gradient_ring(center: Vec2, inner: Vec2, outer: Vec2, c0: Color, c1: Color) {
    const N: u16 = 40;
    let mut vertices = Vec::with_capacity(N as usize * 2);
    let mut indices = Vec::with_capacity(N as usize * 6);
    for i in 0..N {
        let a = i as f32 / N as f32 * std::f32::consts::TAU;
        let d = vec2(a.cos(), a.sin());
        let p0 = center + d * inner;
        let p1 = center + d * outer;
        vertices.push(Vertex::new(p0.x, p0.y, 0.0, 0.0, 0.0, c0));
        vertices.push(Vertex::new(p1.x, p1.y, 0.0, 0.0, 0.0, c1));
        let (j, k) = (i * 2, ((i + 1) % N) * 2);
        indices.extend_from_slice(&[j, j + 1, k + 1, j, k + 1, k]);
    }
    draw_mesh(&Mesh {
        vertices,
        indices,
        texture: None,
    });
}

/// A soft pool of light: `color` at its centre fading to nothing at
/// `rx`×`ry`. Drawn over the ground so lanterns, flasks and fires light
/// the floor around them.
pub fn glow(center: Vec2, rx: f32, ry: f32, color: Color) {
    gradient_ring(
        center,
        Vec2::ZERO,
        vec2(rx, ry),
        color,
        with_alpha(color, 0.0),
    );
}

/// Darkens the screen away from `focus`: clear within `inner`, fading to
/// `NIGHT` at `strength` by `outer`, and that dark beyond. It sets the mood
/// without hiding anything near the player (FR24).
pub fn vignette(focus: Vec2, inner: f32, outer: f32, strength: f32) {
    let clear = with_alpha(NIGHT, 0.0);
    let dark = with_alpha(NIGHT, strength);
    let far = screen_width().max(screen_height()) * 2.0 + outer;
    gradient_ring(focus, Vec2::splat(inner), Vec2::splat(outer), clear, dark);
    gradient_ring(focus, Vec2::splat(outer), Vec2::splat(far), dark, dark);
}

/// A round-bottomed potion flask standing at `base` (the bottom of its
/// bulb), `scale` 1 being the size it sits on the road. `fill` is the
/// potion's colour; `shine` (0..1) brightens its glow. Shared by the road
/// pickup and the "item get" reward so they're the same object.
pub fn potion_flask(base: Vec2, scale: f32, fill: Color, shine: f32, t: f32, alpha: f32) {
    let a = |c: Color| with_alpha(c, c.a * alpha);
    let r = 11.0 * scale;
    let bulb = base - vec2(0.0, r);
    let neck_h = 11.0 * scale;
    let neck_w = 4.2 * scale;
    let neck_top = bulb - vec2(0.0, r * 0.75 + neck_h);
    let glass = Color::new(0.80, 0.90, 1.0, 0.35);

    // Neck, then the bulb, both inked.
    draw_rectangle(
        neck_top.x - neck_w - 1.5,
        neck_top.y,
        (neck_w + 1.5) * 2.0,
        neck_h + r * 0.5,
        a(INK),
    );
    draw_circle(bulb.x, bulb.y, r + 1.6, a(INK));
    draw_rectangle(
        neck_top.x - neck_w,
        neck_top.y,
        neck_w * 2.0,
        neck_h + r * 0.5,
        a(glass),
    );
    draw_circle(bulb.x, bulb.y, r, a(glass));
    // The potion: fills the lower bulb with a sloshing surface.
    let level = bulb.y - r * 0.15 + 1.2 * scale * (t * 3.0).sin();
    let liquid = shade(fill, 0.85 + 0.3 * shine);
    for i in 0..10 {
        let y0 = level + (bulb.y + r - level) * i as f32 / 10.0;
        let y1 = level + (bulb.y + r - level) * (i + 1) as f32 / 10.0;
        let ym = (y0 + y1) * 0.5 - bulb.y;
        let half = (r * r - ym * ym).max(0.0).sqrt() - 0.6;
        draw_rectangle(bulb.x - half, y0, half * 2.0, y1 - y0 + 0.5, a(liquid));
    }
    gfx_surface(bulb.x, level, r, scale, a(shade(fill, 1.25)));
    // Bubbles rising through it.
    for i in 0..3 {
        let u = (t * 0.7 + i as f32 * 0.37).fract();
        let bx = bulb.x + (i as f32 - 1.0) * 4.0 * scale + (t * 3.0 + i as f32).sin() * scale;
        let by = bulb.y + r * 0.8 - u * (bulb.y + r * 0.8 - level);
        draw_circle(
            bx,
            by,
            (1.0 + 0.6 * i as f32) * scale * 0.7,
            a(with_alpha(WHITE, 0.55 * (1.0 - u))),
        );
    }
    // Cork and a lip on the neck.
    draw_rectangle(
        neck_top.x - neck_w - 1.0,
        neck_top.y - 1.0,
        (neck_w + 1.0) * 2.0,
        2.4 * scale,
        a(shade(MOON, 0.9)),
    );
    draw_rectangle(
        neck_top.x - neck_w * 0.8,
        neck_top.y - 5.0 * scale,
        neck_w * 1.6,
        5.0 * scale,
        a(Color::new(0.62, 0.42, 0.26, 1.0)),
    );
    // Moonlit highlight on the glass.
    ellipse_rot(
        bulb + vec2(-r * 0.45, -r * 0.4),
        r * 0.22,
        r * 0.42,
        25.0,
        a(with_alpha(WHITE, 0.7)),
    );
}

fn gfx_surface(cx: f32, y: f32, r: f32, scale: f32, color: Color) {
    ellipse(vec2(cx, y), r * 0.82, 1.6 * scale, color);
}

pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    draw_rectangle(x + r, y, w - 2.0 * r, h, color);
    draw_rectangle(x, y + r, w, h - 2.0 * r, color);
    for (cx, cy) in [
        (x + r, y + r),
        (x + w - r, y + r),
        (x + r, y + h - r),
        (x + w - r, y + h - r),
    ] {
        draw_circle(cx, cy, r, color);
    }
}

/// A floating keyboard key ("press E") hovering with its bottom edge at
/// `bottom`, in screen space. Same parchment-and-dark-wood palette as the egui
/// panels so the in-world prompt and the HUD read as one UI.
pub fn key_prompt(bottom: Vec2, label: &str, t: f32) {
    const SIZE: f32 = 24.0;
    const DEPTH: f32 = 4.0;
    const R: f32 = 5.0;
    const FONT: u16 = 22;
    let face = Color::new(1.0, 0.97, 0.87, 1.0);
    let side = Color::new(0.84, 0.63, 0.35, 1.0);
    let text = Color::new(0.29, 0.19, 0.09, 1.0);

    let bob = (t * 4.0).sin() * 2.0;
    let x = bottom.x - SIZE * 0.5;
    let y = bottom.y - SIZE - DEPTH + bob;
    rounded_rect(
        x - 2.0,
        y - 2.0,
        SIZE + 4.0,
        SIZE + DEPTH + 4.0,
        R + 2.0,
        INK,
    );
    rounded_rect(x, y, SIZE, SIZE + DEPTH, R, side);
    rounded_rect(x, y, SIZE, SIZE, R, face);

    let dims = measure_text(label, None, FONT, 1.0);
    draw_text(
        label,
        (bottom.x - dims.width * 0.5).round(),
        (y + (SIZE + dims.offset_y) * 0.5).round(),
        FONT as f32,
        text,
    );
}

/// Stateless hash -> [0, 1). Lets per-plank / per-frame variation stay
/// stable across frames without storing anything.
pub fn hash01(seed: u64) -> f32 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

/// A heart icon centered on `c`, `size` px across. `fill` is 0 (empty),
/// 1 (left half) or 2 (full), in half-hearts.
pub fn heart(c: Vec2, size: f32, fill: u32) {
    let red = Color::new(0.93, 0.22, 0.28, 1.0);
    let empty = Color::new(0.25, 0.12, 0.10, 0.45);
    heart_shape(c, size * 0.5 + 2.0, INK, true);
    heart_shape(c, size * 0.5, empty, true);
    if fill > 0 {
        heart_shape(c, size * 0.5, red, fill >= 2);
        // Shine, so it reads as a glossy pickup rather than a flat shape.
        let r = size * 0.25;
        ellipse(
            c + vec2(-r, -r * 0.55),
            r * 0.42,
            r * 0.28,
            Color::new(1.0, 1.0, 1.0, 0.55),
        );
    }
}

/// Heart from two circles and a point. `half_w` is half its width;
/// `full` false draws only the left half.
fn heart_shape(c: Vec2, half_w: f32, color: Color, full: bool) {
    let r = half_w * 0.5;
    let top = c.y - r * 0.35;
    draw_circle(c.x - r, top, r, color);
    let left = vec2(c.x - 2.0 * r * 0.96, top + r * 0.35);
    let tip = vec2(c.x, c.y + r * 1.75);
    let mid = vec2(c.x, top + r * 0.35);
    draw_triangle(left, mid, tip, color);
    if full {
        draw_circle(c.x + r, top, r, color);
        draw_triangle(mid, vec2(c.x + 2.0 * r * 0.96, top + r * 0.35), tip, color);
    } else {
        // Cover the gap the missing right circle leaves at the top center.
        draw_rectangle(c.x - r * 0.2, top - r * 0.2, r * 0.2, r * 0.6, color);
    }
}

/// The "item get" moment (FR9): the reward (a potion flask, FR24) held
/// up at `at`, with turning light rays, a burst of sparkles and a banner
/// that rises and fades. `t` is seconds since it started.
pub fn item_get(at: Vec2, t: f32, banner: &str, reduce_flashing: bool) {
    let gold = Color::new(1.0, 0.86, 0.30, 1.0);
    let fade = 1.0 - ((t - 1.2) / 0.4).clamp(0.0, 1.0);
    let pop = (t / 0.18).min(1.0);
    let c = at - vec2(0.0, 6.0 * pop);

    // Rays turning slowly behind the reward.
    let spin = if reduce_flashing { 0.0 } else { t * 1.2 };
    for i in 0..8 {
        let a = spin + i as f32 / 8.0 * std::f32::consts::TAU;
        let d = vec2(a.cos(), a.sin());
        let p = vec2(-d.y, d.x);
        draw_triangle(
            c + p * 2.0,
            c - p * 2.0,
            c + d * 30.0 * pop + p * 6.0,
            with_alpha(gold, 0.35 * fade),
        );
    }
    // The reward: the potion flask, held up and glowing.
    glow(
        c,
        26.0 * pop,
        26.0 * pop,
        with_alpha(LAB_GREEN, 0.35 * fade),
    );
    potion_flask(
        c + vec2(0.0, 12.0 * pop),
        0.9 * pop,
        LAB_GREEN,
        1.0,
        t,
        fade,
    );
    // Sparkle burst flying outward.
    if t < 0.9 {
        let u = t / 0.9;
        for i in 0..7 {
            let a = i as f32 / 7.0 * std::f32::consts::TAU + 0.3;
            let p = c + vec2(a.cos(), a.sin()) * (10.0 + 34.0 * u);
            star(
                p,
                4.5 * (1.0 - u) + 1.0,
                1.6,
                4,
                a,
                1.0,
                with_alpha(gold, 1.0 - u),
            );
        }
    }
    // The banner.
    let rise = (t * 14.0).min(14.0);
    let size = 24u16;
    let dim = measure_text(banner, None, size, 1.0);
    let (w, h) = (dim.width + 22.0, 28.0);
    let top_left = c - vec2(w * 0.5, 50.0 + rise);
    draw_rectangle(
        top_left.x + 3.0,
        top_left.y + 3.0,
        w,
        h,
        with_alpha(INK, 0.6 * fade),
    );
    draw_rectangle(
        top_left.x,
        top_left.y,
        w,
        h,
        Color::new(1.0, 0.97, 0.87, fade),
    );
    draw_rectangle_lines(top_left.x, top_left.y, w, h, 3.0, with_alpha(INK, fade));
    draw_text(
        banner,
        top_left.x + 11.0,
        top_left.y + 20.0,
        size as f32,
        Color::new(0.78, 0.40, 0.10, fade),
    );
}
