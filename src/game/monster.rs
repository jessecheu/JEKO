//! The monster roster (FR10): who can turn up in an encounter, how tough
//! they are, and how each one is drawn and animated. All original beach
//! creatures, drawn with the same flat primitives as the rest of the world.

use super::gfx::{self, INK};
use macroquad::prelude::*;
use std::cell::OnceCell;
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonsterKind {
    /// Hermit crab. Common, one hit: the tutorial monster.
    Pinchling,
    /// Jellyfish. Its zap stuns (FR12).
    Jellyglow,
    /// Pufferfish. Every wrong answer puffs it up and makes it hit harder.
    Puffspike,
    /// Armored coconut. Takes three hits; its bonk stuns.
    CocoKnight,
    /// Seagull in a bandit mask. Its first hit steals the player's hat,
    /// which it wears until it's beaten.
    GullBandit,
    /// Sand worm. Rare, far up the road: a mini-boss.
    DuneWyrm,
    /// A towering wave with eyes. Never random: it guards every 500 m
    /// milestone, with harder questions.
    TideTitan,
}

/// Gameplay numbers for one kind of monster.
#[derive(Clone, Copy, Debug)]
pub struct MonsterInfo {
    pub name: &'static str,
    /// Correct answers needed to defeat it.
    pub hits: u32,
    /// Half-hearts a hit takes off the player.
    pub damage: u32,
    /// Its attack stuns the player for this long (seconds), if at all.
    pub stun_s: f32,
    /// Every wrong answer adds a half-heart to its damage.
    pub grows: bool,
    /// Its first hit steals the player's hat until it's beaten.
    pub steals_hat: bool,
    /// Only appears at milestones, never as a random encounter.
    pub boss: bool,
    /// The earliest distance up the road it can appear.
    pub min_m: u32,
    /// Added to the question difficulty when fighting it.
    pub difficulty_bias: u32,
    /// XP for each correct answer against it.
    pub xp_per_hit: u32,
}

impl MonsterKind {
    pub const ALL: [MonsterKind; 7] = [
        MonsterKind::Pinchling,
        MonsterKind::Jellyglow,
        MonsterKind::Puffspike,
        MonsterKind::CocoKnight,
        MonsterKind::GullBandit,
        MonsterKind::DuneWyrm,
        MonsterKind::TideTitan,
    ];

    pub fn info(self) -> MonsterInfo {
        match self {
            MonsterKind::Pinchling => MonsterInfo {
                name: "Pinchling",
                hits: 1,
                damage: 1,
                stun_s: 0.0,
                grows: false,
                min_m: 0,
                difficulty_bias: 0,
                xp_per_hit: 15,
                steals_hat: false,
                boss: false,
            },
            MonsterKind::Jellyglow => MonsterInfo {
                name: "Jellyglow",
                hits: 1,
                damage: 1,
                stun_s: 2.5,
                grows: false,
                min_m: 30,
                difficulty_bias: 0,
                xp_per_hit: 20,
                steals_hat: false,
                boss: false,
            },
            MonsterKind::Puffspike => MonsterInfo {
                name: "Puffspike",
                hits: 2,
                damage: 1,
                stun_s: 0.0,
                grows: true,
                min_m: 70,
                difficulty_bias: 1,
                xp_per_hit: 20,
                steals_hat: false,
                boss: false,
            },
            MonsterKind::CocoKnight => MonsterInfo {
                name: "Coco Knight",
                hits: 3,
                damage: 2,
                stun_s: 2.0,
                grows: false,
                min_m: 140,
                difficulty_bias: 1,
                xp_per_hit: 25,
                steals_hat: false,
                boss: false,
            },
            MonsterKind::GullBandit => MonsterInfo {
                name: "Gull Bandit",
                hits: 2,
                damage: 1,
                stun_s: 0.0,
                grows: false,
                min_m: 50,
                difficulty_bias: 0,
                xp_per_hit: 20,
                steals_hat: true,
                boss: false,
            },
            MonsterKind::DuneWyrm => MonsterInfo {
                name: "Dune Wyrm",
                hits: 3,
                damage: 2,
                stun_s: 0.0,
                grows: false,
                min_m: 400,
                difficulty_bias: 2,
                xp_per_hit: 30,
                steals_hat: false,
                boss: false,
            },
            MonsterKind::TideTitan => MonsterInfo {
                name: "Tide Titan",
                hits: 4,
                damage: 2,
                stun_s: 0.0,
                grows: false,
                min_m: 500,
                difficulty_bias: 2,
                xp_per_hit: 40,
                steals_hat: false,
                boss: true,
            },
        }
    }

    /// How likely this kind is, `meters` up the road. Pinchlings thin out
    /// and the tough ones grow common the further the player gets.
    fn weight(self, meters: u32) -> f32 {
        let m = meters as f32;
        if meters < self.info().min_m || self.info().boss {
            return 0.0;
        }
        match self {
            MonsterKind::Pinchling => (1.0 - m / 600.0).max(0.15),
            MonsterKind::Jellyglow => 0.8,
            MonsterKind::Puffspike => 0.5 + m / 1000.0,
            MonsterKind::CocoKnight => 0.3 + m / 500.0,
            MonsterKind::GullBandit => 0.6,
            // Rare: a mini-boss.
            MonsterKind::DuneWyrm => 0.12 + m / 8000.0,
            MonsterKind::TideTitan => 0.0,
        }
    }

    /// Picks a kind for an encounter `meters` up the road from a
    /// deterministic `roll` in [0, 1).
    pub fn pick(roll: f32, meters: u32) -> MonsterKind {
        let total: f32 = Self::ALL.iter().map(|k| k.weight(meters)).sum();
        let mut x = roll.clamp(0.0, 0.9999) * total;
        for k in Self::ALL {
            let w = k.weight(meters);
            if x < w {
                return k;
            }
            x -= w;
        }
        MonsterKind::Pinchling
    }
}

/// Which animation is playing; the `f32` is its progress, 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pose {
    Idle,
    Intro(f32),
    Attack(f32),
    Hurt(f32),
    Faint(f32),
}

/// Off-screen canvas the monster is drawn into, so a hurt flash or faint
/// fade tints it as one piece.
pub struct MonsterCanvas(OnceCell<RenderTarget>);

const CANVAS_W: f32 = 120.0;
const CANVAS_H: f32 = 110.0;
/// Ground point inside the canvas.
const CANVAS_OX: f32 = 60.0;
const CANVAS_OY: f32 = 96.0;
const CANVAS_RES: f32 = 2.0;

impl Default for MonsterCanvas {
    fn default() -> Self {
        Self(OnceCell::new())
    }
}

/// Draws `kind` standing at screen point `ground`, `scale` times its
/// natural size. `lunge` is where its attack reaches toward (the player),
/// relative to `ground`. `grown` puffs up a Puffspike.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    canvas: &MonsterCanvas,
    kind: MonsterKind,
    ground: Vec2,
    scale: f32,
    pose: Pose,
    lunge: Vec2,
    grown: u32,
    t: f32,
    reduce_flashing: bool,
) {
    let ease = |u: f32| 1.0 - (1.0 - u).powi(3);
    let (mut offset, mut sx, mut sy, mut rot, mut tint) =
        (Vec2::ZERO, 1.0f32, 1.0f32, 0.0f32, WHITE);
    let (mut atk, mut hurt) = (0.0f32, 0.0f32);
    match pose {
        Pose::Idle => {}
        Pose::Intro(u) => {
            // Bursts in from the right edge and lands with a little pop.
            offset.x = (1.0 - ease(u)) * 420.0;
            let pop = (u * PI).sin() * 0.12;
            sx += pop;
            sy -= pop;
        }
        Pose::Attack(u) => {
            atk = (u * PI).sin();
            offset = lunge * atk.powf(0.7) * 0.75;
        }
        Pose::Hurt(u) => {
            hurt = 1.0 - u;
            offset.x = (u * 55.0).sin() * 7.0 * hurt;
            let flash = if reduce_flashing {
                true
            } else {
                (u * 18.0).sin() > 0.0
            };
            if flash {
                tint = Color::new(1.0, 0.55, 0.55, 1.0);
            }
        }
        Pose::Faint(u) => {
            hurt = 1.0;
            sy = 1.0 - 0.6 * ease(u);
            sx = 1.0 + 0.25 * ease(u);
            rot = 0.0;
            tint.a = 1.0 - u;
        }
    }

    let rt = canvas.0.get_or_init(|| {
        let rt = render_target(
            (CANVAS_W * CANVAS_RES) as u32,
            (CANVAS_H * CANVAS_RES) as u32,
        );
        rt.texture.set_filter(FilterMode::Linear);
        rt
    });
    let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, CANVAS_W, CANVAS_H));
    cam.render_target = Some(rt.clone());
    set_camera(&cam);
    clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
    let o = vec2(CANVAS_OX, CANVAS_OY);
    // Ground shadow inside the canvas so it fades with the faint.
    gfx::ellipse(
        o + vec2(0.0, 2.0),
        26.0,
        6.0,
        Color::new(0.0, 0.0, 0.0, 0.22),
    );
    match kind {
        MonsterKind::Pinchling => draw_pinchling(o, t, atk, hurt),
        MonsterKind::Jellyglow => draw_jellyglow(o, t, atk, hurt),
        MonsterKind::Puffspike => draw_puffspike(o, t, atk, hurt, grown),
        MonsterKind::CocoKnight => draw_coco_knight(o, t, atk, hurt),
        MonsterKind::GullBandit => draw_gull_bandit(o, t, atk, hurt, grown > 0),
        MonsterKind::DuneWyrm => draw_dune_wyrm(o, t, atk, hurt),
        MonsterKind::TideTitan => draw_tide_titan(o, t, atk, hurt),
    }
    set_default_camera();

    let foot = ground + offset;
    draw_texture_ex(
        &rt.texture,
        foot.x - CANVAS_OX * scale * sx,
        foot.y - CANVAS_OY * scale * sy,
        tint,
        DrawTextureParams {
            dest_size: Some(vec2(CANVAS_W * scale * sx, CANVAS_H * scale * sy)),
            rotation: rot,
            pivot: Some(foot),
            flip_y: true,
            ..Default::default()
        },
    );
}

/// Two cartoon eyes looking left (toward the player). `shut` squeezes them
/// into "><" when hurt.
fn eyes(a: Vec2, b: Vec2, r: f32, shut: bool) {
    for p in [a, b] {
        if shut {
            gfx::line(p + vec2(-r, -r * 0.7), p + vec2(r * 0.6, 0.0), 1.6, INK);
            gfx::line(p + vec2(-r, r * 0.7), p + vec2(r * 0.6, 0.0), 1.6, INK);
        } else {
            draw_circle(p.x, p.y, r, WHITE);
            draw_circle_lines(p.x, p.y, r, 1.3, INK);
            draw_circle(p.x - r * 0.35, p.y + r * 0.1, r * 0.55, INK);
            draw_circle(p.x - r * 0.5, p.y - r * 0.25, r * 0.2, WHITE);
        }
    }
}

fn draw_pinchling(o: Vec2, t: f32, atk: f32, hurt: f32) {
    let red = Color::new(0.92, 0.36, 0.28, 1.0);
    let shell = Color::new(0.96, 0.72, 0.48, 1.0);
    let scuttle = (t * 2.2).sin() * 5.0;
    let base = o + vec2(scuttle, 0.0);
    let hiding = hurt > 0.2;

    if !hiding {
        // Legs pattering.
        for i in 0..3 {
            let fi = i as f32;
            let phase = (t * 12.0 + fi * 2.0).sin() * 2.5;
            for side in [-1.0f32, 1.0] {
                let hip = base + vec2(side * (6.0 + fi * 5.0), -10.0);
                let toe = base + vec2(side * (12.0 + fi * 6.0), -1.0 + phase * side);
                gfx::line(hip, toe, 2.4, INK);
                gfx::line(hip, toe, 1.4, red);
            }
        }
        // Claws, snapping open on the attack.
        for (side, size) in [(-1.0f32, 1.25f32), (1.0, 0.9)] {
            let c = base + vec2(side * 24.0 - atk * 10.0, -16.0 - atk * 6.0);
            let open = 0.35 + atk * 0.9 + (t * 3.0).sin().max(0.0) * 0.15;
            gfx::ellipse(c, 7.5 * size, 5.5 * size, red);
            gfx::ellipse_outline(c, 7.5 * size, 5.5 * size, 1.4, INK);
            let tip = c + vec2(-6.0 * size, -open * 6.0);
            draw_triangle(c + vec2(-2.0, -2.0), tip, c + vec2(-8.0 * size, 1.0), red);
            gfx::line(c + vec2(-2.0, -2.0), tip, 1.2, INK);
        }
        // Eyestalks.
        for dx in [-6.0f32, 2.0] {
            let top = base + vec2(dx - 4.0, -36.0 + (t * 3.0 + dx).sin());
            gfx::line(base + vec2(dx - 2.0, -20.0), top, 2.2, INK);
            gfx::line(base + vec2(dx - 2.0, -20.0), top, 1.2, red);
        }
        eyes(
            base + vec2(-10.0, -38.0),
            base + vec2(-2.0, -37.0),
            3.6,
            false,
        );
        gfx::ellipse(base + vec2(-4.0, -14.0), 12.0, 7.0, red);
        gfx::ellipse_outline(base + vec2(-4.0, -14.0), 12.0, 7.0, 1.4, INK);
    }

    // The spiral shell on its back; it hides inside when hurt.
    let sc = base + vec2(8.0, -22.0 + if hiding { 8.0 } else { 0.0 });
    gfx::ellipse(sc, 18.0, 16.0, shell);
    gfx::ellipse_outline(sc, 18.0, 16.0, 1.8, INK);
    let mut pts = Vec::new();
    for i in 0..28 {
        let a = i as f32 * 0.42;
        let r = 14.0 * (1.0 - i as f32 / 30.0);
        pts.push(sc + vec2(a.cos() * r, a.sin() * r * 0.85));
    }
    gfx::polyline(&pts, 1.5, gfx::shade(shell, 0.7));
}

fn draw_jellyglow(o: Vec2, t: f32, atk: f32, hurt: f32) {
    let body = Color::new(0.80, 0.62, 0.98, 1.0);
    let float = (t * 2.0).sin() * 5.0;
    let pulse = 1.0 + (t * 3.0).sin() * 0.05;
    let c = o + vec2(0.0, -52.0 + float);

    if atk > 0.25 {
        // The zap: a bright halo and crackling sparks.
        draw_circle(
            c.x,
            c.y,
            34.0 * atk,
            Color::new(1.0, 0.95, 0.45, 0.45 * atk),
        );
        for i in 0..6 {
            let a = i as f32 / 6.0 * TAU + t * 4.0;
            let p = c + vec2(a.cos(), a.sin()) * 30.0 * atk;
            gfx::star(p, 4.0, 2.0, 5, a, 1.0, Color::new(1.0, 0.9, 0.3, 1.0));
        }
    }

    // Tentacles sway.
    for i in 0..5 {
        let x = -14.0 + i as f32 * 7.0;
        let pts: Vec<Vec2> = (0..8)
            .map(|j| {
                let fj = j as f32;
                c + vec2(
                    x + (t * 3.0 + fj * 0.7 + i as f32).sin() * 3.0,
                    6.0 + fj * 5.0,
                )
            })
            .collect();
        gfx::polyline(&pts, 3.0, INK);
        gfx::polyline(&pts, 1.8, gfx::shade(body, 0.9));
    }
    // Dome.
    gfx::ellipse(
        c + vec2(0.0, 6.0),
        22.0 * pulse,
        5.0,
        gfx::shade(body, 0.85),
    );
    draw_circle(c.x, c.y, 21.0 * pulse, body);
    draw_rectangle(c.x - 22.0 * pulse, c.y, 44.0 * pulse, 7.0, body);
    draw_arc(c.x, c.y, 24, 21.0 * pulse, 180.0, 1.8, 180.0, INK);
    gfx::line(
        c + vec2(-22.0 * pulse, 0.0),
        c + vec2(-22.0 * pulse, 7.0),
        1.8,
        INK,
    );
    gfx::line(
        c + vec2(22.0 * pulse, 0.0),
        c + vec2(22.0 * pulse, 7.0),
        1.8,
        INK,
    );
    gfx::ellipse(
        c + vec2(-8.0, -10.0),
        6.0,
        3.5,
        Color::new(1.0, 1.0, 1.0, 0.6),
    );
    eyes(c + vec2(-8.0, -1.0), c + vec2(4.0, -1.0), 3.8, hurt > 0.2);
    gfx::ellipse(
        c + vec2(-12.0, 5.0),
        2.6,
        1.6,
        Color::new(1.0, 0.55, 0.7, 0.7),
    );
}

fn draw_puffspike(o: Vec2, t: f32, atk: f32, hurt: f32, grown: u32) {
    let skin = Color::new(0.98, 0.80, 0.30, 1.0);
    // Sits in a little tide pool.
    gfx::ellipse(
        o + vec2(0.0, -2.0),
        34.0,
        8.0,
        Color::new(0.35, 0.72, 0.88, 0.9),
    );
    gfx::ellipse_outline(o + vec2(0.0, -2.0), 34.0, 8.0, 1.4, gfx::shade(skin, 0.6));
    let bob = (t * 2.4).sin() * 2.0;
    let size = (1.0 + grown as f32 * 0.15).min(1.6);
    let puff = 1.0 + atk * 0.35 - hurt * 0.2;
    let r = 15.0 * size * puff;
    let c = o + vec2(0.0, -r - 2.0 + bob);

    // Tail fin.
    let wag = (t * 6.0).sin() * 3.0;
    draw_triangle(
        c + vec2(r - 2.0, 0.0),
        c + vec2(r + 12.0, -8.0 + wag),
        c + vec2(r + 12.0, 8.0 + wag),
        gfx::shade(skin, 0.85),
    );
    // Spikes all round.
    for i in 0..14 {
        let a = i as f32 / 14.0 * TAU;
        let d = vec2(a.cos(), a.sin());
        let p = vec2(-d.y, d.x);
        draw_triangle(
            c + d * (r - 1.0) + p * 3.0,
            c + d * (r - 1.0) - p * 3.0,
            c + d * (r + 6.0 + atk * 4.0),
            gfx::shade(skin, 0.7),
        );
    }
    draw_circle(c.x, c.y, r, skin);
    draw_circle_lines(c.x, c.y, r, 1.8, INK);
    gfx::ellipse(
        c + vec2(2.0, r * 0.4),
        r * 0.7,
        r * 0.4,
        gfx::shade(skin, 1.12),
    );
    eyes(
        c + vec2(-r * 0.45, -r * 0.25),
        c + vec2(r * 0.05, -r * 0.3),
        3.8,
        hurt > 0.2,
    );
    // Little "o" mouth.
    draw_circle_lines(c.x - r * 0.6, c.y + r * 0.2, 2.2, 1.4, INK);
}

fn draw_coco_knight(o: Vec2, t: f32, atk: f32, hurt: f32) {
    let husk = Color::new(0.55, 0.35, 0.20, 1.0);
    let steel = Color::new(0.72, 0.76, 0.82, 1.0);
    let step = (t * 5.0).sin();
    let lean = -atk * 0.35;
    let c = o + vec2(0.0, -26.0 + step.abs() * -2.0);

    // Stubby legs.
    for (side, ph) in [(-1.0f32, step), (1.0, -step)] {
        let hip = c + vec2(side * 8.0, 16.0);
        let foot = o + vec2(side * 9.0 + ph * 2.0, -1.0);
        gfx::line(hip, foot, 4.0, INK);
        gfx::line(hip, foot, 2.6, gfx::shade(husk, 0.8));
        gfx::ellipse(foot, 4.0, 2.2, INK);
    }
    // The coconut, leaning into its head-butt.
    let head = c + vec2(lean * 30.0, 0.0);
    draw_circle(head.x, head.y, 21.0, husk);
    draw_circle_lines(head.x, head.y, 21.0, 2.0, INK);
    for i in 0..7 {
        let a = -2.6 + i as f32 * 0.35;
        let p = head + vec2(a.cos(), a.sin()) * 17.0;
        gfx::line(
            p,
            p + vec2(a.cos(), a.sin()) * 5.0,
            1.0,
            gfx::shade(husk, 0.7),
        );
    }
    // Helmet visor band with an eye slit.
    let band = head + vec2(0.0, -3.0);
    draw_rectangle(band.x - 21.0, band.y - 6.0, 42.0, 12.0, steel);
    draw_rectangle_lines(band.x - 21.0, band.y - 6.0, 42.0, 12.0, 1.6, INK);
    draw_rectangle(band.x - 16.0, band.y - 2.0, 24.0, 4.0, INK);
    let glint = if hurt > 0.2 {
        RED
    } else {
        Color::new(1.0, 0.95, 0.6, 1.0)
    };
    draw_circle(band.x - 11.0, band.y, 1.6, glint);
    draw_circle(band.x - 3.0, band.y, 1.6, glint);
    // A plume, and cracks when hurt.
    let plume = head + vec2(4.0, -21.0);
    gfx::ellipse_rot(
        plume + vec2(4.0, -5.0),
        9.0,
        4.0,
        -30.0 + (t * 4.0).sin() * 8.0,
        RED,
    );
    if hurt > 0.0 {
        gfx::polyline(
            &[
                head + vec2(6.0, -18.0),
                head + vec2(2.0, -12.0),
                head + vec2(8.0, -9.0),
                head + vec2(4.0, -6.0),
            ],
            1.6,
            INK,
        );
    }
}

/// The player's straw hat, for the Gull Bandit to wear once it's stolen.
fn stolen_hat(c: Vec2, tilt: f32) {
    let straw = Color::new(0.97, 0.84, 0.48, 1.0);
    gfx::ellipse_rot(c, 13.0, 4.0, tilt, straw);
    gfx::ellipse(c + vec2(0.0, -4.0), 7.0, 5.0, straw);
    gfx::ellipse(
        c + vec2(0.0, -2.0),
        7.0,
        1.6,
        Color::new(0.90, 0.30, 0.30, 1.0),
    );
    gfx::ellipse_outline(c + vec2(0.0, -4.0), 7.0, 5.0, 1.4, INK);
}

fn draw_gull_bandit(o: Vec2, t: f32, atk: f32, hurt: f32, has_hat: bool) {
    let white = Color::new(0.98, 0.98, 0.96, 1.0);
    let grey = Color::new(0.62, 0.66, 0.72, 1.0);
    let orange = Color::new(0.98, 0.62, 0.20, 1.0);
    let hop = ((t * 3.0).sin().max(0.0)) * 6.0;
    let c = o + vec2(0.0, -30.0 - hop - atk * 10.0);

    // Legs.
    for dx in [-5.0f32, 5.0] {
        gfx::line(c + vec2(dx, 10.0), o + vec2(dx, -1.0 - hop), 2.0, orange);
    }
    // Wings: folded at rest, spread wide on the swoop.
    let spread = atk.max(((t * 9.0).sin() * 0.5 + 0.5) * 0.25);
    for side in [-1.0f32, 1.0] {
        let tip = c + vec2(side * (12.0 + spread * 22.0), -4.0 - spread * 18.0);
        draw_triangle(
            c + vec2(side * 4.0, -6.0),
            c + vec2(side * 6.0, 6.0),
            tip,
            grey,
        );
        gfx::line(c + vec2(side * 4.0, -6.0), tip, 1.4, INK);
    }
    gfx::ellipse(c, 14.0, 11.0, white);
    gfx::ellipse_outline(c, 14.0, 11.0, 1.8, INK);
    let head = c + vec2(-8.0, -14.0);
    draw_circle(head.x, head.y, 9.0, white);
    draw_circle_lines(head.x, head.y, 9.0, 1.8, INK);
    // Beak.
    draw_triangle(
        head + vec2(-7.0, -1.0),
        head + vec2(-7.0, 3.0),
        head + vec2(-17.0, 2.0 + atk * 2.0),
        orange,
    );
    // Bandit mask with eye holes.
    draw_rectangle(head.x - 9.0, head.y - 4.0, 16.0, 5.0, INK);
    eyes(
        head + vec2(-5.0, -1.5),
        head + vec2(1.0, -1.5),
        2.0,
        hurt > 0.2,
    );
    if has_hat {
        stolen_hat(head + vec2(1.0, -9.0), -12.0);
    }
    // Feathers fly when it's hit.
    if hurt > 0.0 {
        for i in 0..5 {
            let a = i as f32 * 1.3;
            let d = (1.0 - hurt) * 30.0 + 6.0;
            gfx::ellipse_rot(
                c + vec2(a.cos() * d, a.sin() * d - 8.0),
                4.0,
                1.6,
                a.to_degrees(),
                gfx::with_alpha(white, hurt),
            );
        }
    }
}

fn draw_dune_wyrm(o: Vec2, t: f32, atk: f32, hurt: f32) {
    let sand = Color::new(0.90, 0.76, 0.50, 1.0);
    let skin = Color::new(0.78, 0.55, 0.40, 1.0);
    // Mound it bursts out of.
    gfx::ellipse(o + vec2(0.0, -3.0), 36.0, 10.0, sand);
    gfx::ellipse_outline(o + vec2(0.0, -3.0), 36.0, 10.0, 1.6, gfx::shade(sand, 0.7));
    // Segmented body swaying up out of the sand, rearing on the attack.
    let rear = 1.0 + atk * 0.25;
    let n = 7;
    let mut head = o;
    for i in 0..n {
        let k = i as f32 / (n - 1) as f32;
        let sway = (t * 2.0 - k * 2.2).sin() * 10.0 * k;
        let p = o + vec2(sway - atk * k * 18.0, -6.0 - k * 70.0 * rear);
        let r = 13.0 - k * 3.0;
        draw_circle(p.x, p.y, r + 1.8, INK);
        draw_circle(
            p.x,
            p.y,
            r,
            if i % 2 == 0 {
                skin
            } else {
                gfx::shade(skin, 1.1)
            },
        );
        head = p;
    }
    // Head with mandibles that gape on the attack.
    let gape = 0.3 + atk * 0.8;
    for side in [-1.0f32, 1.0] {
        gfx::line(
            head + vec2(-6.0, 4.0 * side),
            head + vec2(-16.0, 4.0 * side + side * gape * 8.0),
            3.0,
            INK,
        );
    }
    eyes(
        head + vec2(-5.0, -5.0),
        head + vec2(3.0, -6.0),
        3.4,
        hurt > 0.2,
    );
    // Sand spray as it erupts.
    for i in 0..6 {
        let a = -2.6 + i as f32 * 0.25;
        let d = 20.0 + ((t * 4.0 + i as f32).sin() * 0.5 + 0.5) * 10.0;
        draw_circle(o.x + a.cos() * d, o.y - 6.0 + a.sin() * d * 0.5, 2.0, sand);
    }
}

fn draw_tide_titan(o: Vec2, t: f32, atk: f32, hurt: f32) {
    let deep = Color::new(0.10, 0.48, 0.74, 1.0);
    let mid = Color::new(0.22, 0.66, 0.86, 1.0);
    let foam = Color::new(0.94, 0.99, 1.0, 1.0);
    let sway = (t * 1.5).sin() * 4.0;
    let lean = atk * 22.0;
    // Body: stacked swells narrowing up to a curling crest.
    for i in 0..6 {
        let k = i as f32 / 5.0;
        let c = o + vec2(sway * k - lean * k * k, -8.0 - k * 72.0);
        let rx = 40.0 - k * 16.0;
        gfx::ellipse(c, rx + 2.0, 12.0, INK);
        gfx::ellipse(c, rx, 10.5, if i % 2 == 0 { deep } else { mid });
    }
    let crest = o + vec2(sway - lean - 8.0, -86.0);
    gfx::ellipse(crest, 22.0, 9.0, foam);
    gfx::ellipse_outline(crest, 22.0, 9.0, 1.6, INK);
    for i in 0..5 {
        let p = crest
            + vec2(
                -20.0 + i as f32 * 9.0,
                6.0 + (t * 5.0 + i as f32).sin() * 2.0,
            );
        draw_circle(p.x, p.y, 3.0, foam);
    }
    // Face.
    let face = o + vec2(sway * 0.5 - lean * 0.4, -44.0);
    eyes(
        face + vec2(-12.0, -6.0),
        face + vec2(6.0, -7.0),
        5.5,
        hurt > 0.2,
    );
    draw_arc(face.x - 3.0, face.y + 9.0, 16, 7.0, 200.0, 2.0, 140.0, INK);
    // Spray on the crash.
    if atk > 0.3 {
        for i in 0..8 {
            let a = -3.0 + i as f32 * 0.3;
            let p = crest + vec2(a.cos(), a.sin()) * 26.0 * atk;
            draw_circle(p.x, p.y, 2.4, gfx::with_alpha(foam, atk));
        }
    }
}
