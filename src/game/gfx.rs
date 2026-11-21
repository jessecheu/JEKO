//! Small drawing vocabulary shared by the beach world, its decorations, and
//! the player. Everything in the scene is flat-colored primitives, so these
//! helpers are what keep the "storybook cartoon" look consistent: one ink
//! color for outlines, one warm shadow tint, and the same shading math for
//! every lit/unlit face.

use macroquad::prelude::*;

/// Soft brown ink instead of pure black — reads as a cartoon outline
/// without the harsher, more technical look of true black.
pub const INK: Color = Color::new(0.18, 0.11, 0.05, 0.85);

/// Warm-tinted ground shadow. A brown-ish shadow on sand reads as "sunny
/// afternoon"; a neutral gray one reads as overcast.
pub const SHADOW: Color = Color::new(0.35, 0.20, 0.05, 0.22);

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
/// `center`. Used for towels and planks that shouldn't all sit square to
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

/// Filled n-pointed star as a triangle fan — starfish, glints, sparkles.
/// `squash` < 1 flattens it vertically so something lying on the sand
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
/// at each point — the building block for palm fronds and grass blades,
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

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
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
/// `bottom`, in screen space. Same cream-and-driftwood palette as the egui
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

/// The "item get" moment (FR9): the reward (a message in a bottle) held
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
    // The reward: a corked bottle with a scroll inside.
    let glass = Color::new(0.55, 0.88, 0.86, 0.95 * fade);
    rot_rect(c, vec2(6.5, 10.0) * pop, 0.0, with_alpha(INK, fade));
    rot_rect(c, vec2(5.0, 8.5) * pop, 0.0, glass);
    rot_rect(
        c + vec2(0.0, -11.0) * pop,
        vec2(2.6, 2.6) * pop,
        0.0,
        Color::new(0.7, 0.48, 0.28, fade),
    );
    rot_rect(
        c + vec2(0.0, 1.0) * pop,
        vec2(2.4, 5.0) * pop,
        0.0,
        Color::new(1.0, 0.97, 0.85, fade),
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
