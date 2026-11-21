//! A wild encounter's battle (FR10): a short transition into a battle
//! view, then question after question until the monster faints. A correct
//! answer lands the player's attack; a wrong one lets the monster attack.
//! The questions themselves come through the same `TriggerState` pipeline
//! (and fallback bank) as the potion flasks.

use super::bow;
use super::gfx::{self, MOON};
use super::manor;
use super::monster::{self, MonsterCanvas, MonsterKind, Pose};
use super::player::Player;
use macroquad::prelude::*;

/// Flash, then the iris closes on the player and opens on the battle.
pub const ENTER_S: f32 = 1.0;
const ENTER_FLASH_S: f32 = 0.15;
const ENTER_SWAP_S: f32 = 0.6;
pub const INTRO_S: f32 = 0.8;
pub const PLAYER_ATTACK_S: f32 = 0.8;
/// When in the player's attack the blow lands.
/// When in the player's attack the arrow lands; it leaves the bow at
/// `bow::RELEASE_S`.
pub const PLAYER_HIT_AT: f32 = 0.4;
/// How long the impact star shows on the monster.
const IMPACT_S: f32 = 0.16;
pub const MONSTER_ATTACK_S: f32 = 0.9;
pub const MONSTER_HIT_AT: f32 = 0.4;
pub const FAINT_S: f32 = 1.0;
/// Iris closes on the battle and opens back on the world.
pub const EXIT_S: f32 = 0.7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Enter(f32),
    Intro(f32),
    /// Waiting on the player's answer.
    Question,
    PlayerAttack(f32),
    MonsterAttack(f32),
    Faint(f32),
    Exit(f32),
}

pub struct Battle {
    pub kind: MonsterKind,
    /// Index of the encounter zone in `World::zones` that started it.
    pub zone: usize,
    pub hits_left: u32,
    /// Wrong answers so far (puffs up a Puffspike).
    pub misses: u32,
    pub phase: Phase,
    /// Whether this attack's blow has been applied yet.
    pub hit_landed: bool,
    canvas: MonsterCanvas,
}

impl Battle {
    pub fn new(kind: MonsterKind, zone: usize) -> Self {
        Self {
            kind,
            zone,
            hits_left: kind.info().hits,
            misses: 0,
            phase: Phase::Enter(0.0),
            hit_landed: false,
            canvas: MonsterCanvas::default(),
        }
    }

    /// Half-hearts the monster's next hit takes.
    pub fn damage(&self) -> u32 {
        let info = self.kind.info();
        info.damage + if info.grows { self.misses } else { 0 }
    }

    /// True while the battle view (not the world) is on screen.
    pub fn scene_visible(&self) -> bool {
        match self.phase {
            Phase::Enter(t) => t >= ENTER_SWAP_S,
            Phase::Exit(t) => t < EXIT_S * 0.5,
            _ => true,
        }
    }

    /// The transition overlay for this frame: `(iris radius as a fraction
    /// of the screen diagonal, white flash alpha)`. `None` when no
    /// transition is playing.
    fn transition(&self, reduce_flashing: bool) -> Option<(f32, f32)> {
        match self.phase {
            Phase::Enter(t) => {
                let flash = if reduce_flashing || t > ENTER_FLASH_S {
                    0.0
                } else {
                    0.8 * (1.0 - t / ENTER_FLASH_S)
                };
                let r = if t < ENTER_SWAP_S {
                    1.0 - ((t - ENTER_FLASH_S).max(0.0) / (ENTER_SWAP_S - ENTER_FLASH_S))
                } else {
                    (t - ENTER_SWAP_S) / (ENTER_S - ENTER_SWAP_S)
                };
                Some((r.clamp(0.0, 1.0), flash))
            }
            Phase::Exit(t) => {
                let half = EXIT_S * 0.5;
                let r = if t < half {
                    1.0 - t / half
                } else {
                    (t - half) / half
                };
                Some((r.clamp(0.0, 1.0), 0.0))
            }
            _ => None,
        }
    }

    /// Draws the iris (and flash) over whatever is on screen, centered on
    /// `center`.
    pub fn draw_transition(&self, center: Vec2, reduce_flashing: bool) {
        let Some((r, flash)) = self.transition(reduce_flashing) else {
            return;
        };
        let (w, h) = (screen_width(), screen_height());
        let far = (w * w + h * h).sqrt();
        iris(center, r * far, far * 1.5);
        if flash > 0.0 {
            draw_rectangle(0.0, 0.0, w, h, Color::new(1.0, 1.0, 1.0, flash));
        }
    }

    /// The battle view: backdrop, the player on the left, the monster on
    /// the right with its name and hits left.
    pub fn draw_scene(&self, player: &Player, t: f32, reduce_flashing: bool) {
        let (w, h) = (screen_width(), screen_height());
        draw_backdrop(w, h, t, reduce_flashing);

        let player_at = vec2(w * 0.30, h * 0.66);
        let monster_at = vec2(w * 0.70, h * 0.50);
        for (p, rx) in [(player_at, 70.0), (monster_at, 80.0)] {
            // A mossy stone platform under each fighter, lit from above.
            gfx::glow(
                p + vec2(0.0, 4.0),
                rx * 1.6,
                34.0,
                gfx::with_alpha(MOON, 0.18),
            );
            gfx::ellipse(
                p + vec2(0.0, 4.0),
                rx,
                16.0,
                Color::new(0.34, 0.36, 0.40, 1.0),
            );
            gfx::ellipse(
                p + vec2(-rx * 0.25, 0.0),
                rx * 0.55,
                7.0,
                Color::new(0.24, 0.38, 0.22, 1.0),
            );
            gfx::ellipse_outline(p + vec2(0.0, 4.0), rx, 16.0, 2.0, gfx::INK);
        }

        player.draw_at(player_at, 2.0, reduce_flashing);

        let pose = match self.phase {
            Phase::Enter(_) => Pose::Intro(0.0),
            Phase::Intro(u) => Pose::Intro(u / INTRO_S),
            Phase::Question | Phase::Exit(_) => Pose::Idle,
            Phase::PlayerAttack(u) if u >= PLAYER_HIT_AT => {
                Pose::Hurt(((u - PLAYER_HIT_AT) / (PLAYER_ATTACK_S - PLAYER_HIT_AT)).min(1.0))
            }
            Phase::PlayerAttack(_) => Pose::Idle,
            Phase::MonsterAttack(u) => Pose::Attack(u / MONSTER_ATTACK_S),
            Phase::Faint(u) => Pose::Faint(u / FAINT_S),
        };
        if !matches!(self.phase, Phase::Exit(_)) || self.hits_left > 0 {
            monster::draw(
                &self.canvas,
                self.kind,
                monster_at,
                2.0,
                pose,
                player_at - monster_at,
                self.misses,
                t,
                reduce_flashing,
            );
        }

        if let Phase::PlayerAttack(u) = self.phase {
            draw_shot(player_at, monster_at, u, 2.0);
        }

        // Nameplate with a pip per hit still needed.
        let info = self.kind.info();
        let plate = vec2(monster_at.x - 90.0, h * 0.13);
        draw_rectangle(
            plate.x,
            plate.y,
            180.0,
            44.0,
            Color::new(1.0, 0.97, 0.87, 0.95),
        );
        draw_rectangle_lines(plate.x, plate.y, 180.0, 44.0, 2.5, gfx::INK);
        draw_text(info.name, plate.x + 10.0, plate.y + 19.0, 22.0, gfx::INK);
        for i in 0..info.hits {
            let c = vec2(plate.x + 18.0 + i as f32 * 20.0, plate.y + 32.0);
            let full = i < self.hits_left;
            draw_circle(c.x, c.y, 6.5, gfx::INK);
            draw_circle(
                c.x,
                c.y,
                5.0,
                if full {
                    Color::new(0.95, 0.35, 0.30, 1.0)
                } else {
                    Color::new(0.6, 0.55, 0.5, 1.0)
                },
            );
        }
        let mut traits = Vec::new();
        if info.stun_s > 0.0 {
            traits.push("stuns");
        }
        if info.grows {
            traits.push("grows");
        }
        if info.steals_hat {
            traits.push("thief");
        }
        if info.boss {
            traits.push("BOSS");
        }
        if !traits.is_empty() {
            draw_text(
                traits.join(" · ").as_str(),
                plate.x + 18.0 + info.hits as f32 * 20.0,
                plate.y + 37.0,
                16.0,
                gfx::shade(gfx::INK, 1.6),
            );
        }
    }
}

/// The player's arrow (FR19) from the bow to the monster, `u` seconds into
/// the attack, then a star where it strikes. `scale` matches the sprites'.
fn draw_shot(player_at: Vec2, monster_at: Vec2, u: f32, scale: f32) {
    let from = player_at + bow::NOCK_FROM_FOOT * scale;
    let target = monster_at + vec2(-12.0, -26.0) * scale;
    if (bow::RELEASE_S..PLAYER_HIT_AT).contains(&u) {
        // A shallow arc: up, then down onto the target.
        let k = (u - bow::RELEASE_S) / (PLAYER_HIT_AT - bow::RELEASE_S);
        let arc = 26.0 * scale;
        let at = |k: f32| from.lerp(target, k) - vec2(0.0, (k * std::f32::consts::PI).sin() * arc);
        let along = (at(k + 0.02) - at(k)).normalize_or(vec2(1.0, 0.0));
        let len = bow::ARROW_LEN * scale;
        bow::draw_arrow(at(k) - along * len, along, len, scale, 1.0, true);
    } else if (PLAYER_HIT_AT..PLAYER_HIT_AT + IMPACT_S).contains(&u) {
        let k = (u - PLAYER_HIT_AT) / IMPACT_S;
        let r = (10.0 + 10.0 * k) * scale;
        let gold = Color::new(1.0, 0.86, 0.25, 1.0 - k);
        gfx::star(
            target,
            r + 2.0,
            r * 0.4 + 1.0,
            6,
            0.3,
            1.0,
            gfx::with_alpha(gfx::INK, 1.0 - k),
        );
        gfx::star(target, r, r * 0.4, 6, 0.3, 1.0, gold);
    }
}

/// A moonlit clearing (FR24): a night sky with stars and the moon, the
/// manor on a far hill with lightning behind it, a ragged treeline, and
/// mossy ground for the fighters to stand on.
fn draw_backdrop(w: f32, h: f32, t: f32, reduce_flashing: bool) {
    let horizon = h * 0.40;
    let flash = if reduce_flashing {
        0.0
    } else {
        manor::lightning(t)
    };
    for i in 0..12 {
        let y0 = horizon * i as f32 / 12.0;
        let k = i as f32 / 11.0;
        let sky = gfx::mix(
            Color::new(0.04, 0.04, 0.12, 1.0),
            Color::new(0.20, 0.14, 0.30, 1.0),
            k,
        );
        draw_rectangle(
            0.0,
            y0,
            w,
            horizon / 12.0 + 1.0,
            gfx::mix(sky, MOON, 0.25 * flash),
        );
    }
    // Stars, twinkling slowly (steady under reduce flashing).
    for i in 0..60u64 {
        let x = gfx::hash01(i * 3) * w;
        let y = gfx::hash01(i * 3 + 1) * horizon * 0.85;
        let tw = if reduce_flashing {
            0.7
        } else {
            0.5 + 0.5 * (t * 1.3 + i as f32).sin()
        };
        let r = 0.6 + 1.0 * gfx::hash01(i * 3 + 2);
        draw_circle(x, y, r, gfx::with_alpha(MOON, 0.35 + 0.5 * tw));
    }
    // The moon, with a halo and a couple of craters.
    let moon = vec2(w * 0.86, h * 0.11);
    gfx::glow(moon, 90.0, 90.0, gfx::with_alpha(MOON, 0.25));
    draw_circle(moon.x, moon.y, 30.0, Color::new(0.93, 0.93, 0.86, 1.0));
    draw_circle(
        moon.x - 9.0,
        moon.y - 6.0,
        6.0,
        Color::new(0.82, 0.82, 0.76, 1.0),
    );
    draw_circle(
        moon.x + 8.0,
        moon.y + 9.0,
        4.0,
        Color::new(0.82, 0.82, 0.76, 1.0),
    );

    // The far hill and the manor on it, over on the player's side.
    let hill = Color::new(0.07, 0.07, 0.12, 1.0);
    let hill_c = vec2(w * 0.22, horizon + 10.0);
    gfx::ellipse(hill_c, w * 0.26, 60.0, hill);
    let scale = (h / 600.0).clamp(0.7, 1.2) * 0.75;
    manor::draw(hill_c - vec2(0.0, 44.0), scale, t, false, reduce_flashing);

    // A ragged treeline along the horizon.
    let trees = Color::new(0.04, 0.07, 0.07, 1.0);
    let mut x = -20.0;
    let mut i = 0u64;
    while x < w + 20.0 {
        let th = 34.0 + 46.0 * gfx::hash01(i ^ 0x7EE);
        let tw = 16.0 + 12.0 * gfx::hash01(i ^ 0x7EF);
        draw_triangle(
            vec2(x - tw, horizon + 6.0),
            vec2(x + tw, horizon + 6.0),
            vec2(x, horizon + 6.0 - th),
            trees,
        );
        x += tw * 1.1;
        i += 1;
    }

    // Mossy ground, darker towards the front.
    let ground_h = h - horizon;
    for i in 0..10 {
        let y0 = horizon + 4.0 + ground_h * i as f32 / 10.0;
        let k = i as f32 / 9.0;
        draw_rectangle(
            0.0,
            y0,
            w,
            ground_h / 10.0 + 1.0,
            gfx::mix(
                Color::new(0.12, 0.18, 0.13, 1.0),
                Color::new(0.07, 0.11, 0.08, 1.0),
                k,
            ),
        );
    }
    // A low mist along the treeline.
    gfx::glow(
        vec2(w * 0.5, horizon + 8.0),
        w * 0.7,
        26.0,
        gfx::with_alpha(MOON, 0.12),
    );
}

/// Black everywhere outside a circle of radius `r` around `c`.
fn iris(c: Vec2, r: f32, far: f32) {
    const N: usize = 64;
    for i in 0..N {
        let a0 = i as f32 / N as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / N as f32 * std::f32::consts::TAU;
        let d0 = vec2(a0.cos(), a0.sin());
        let d1 = vec2(a1.cos(), a1.sin());
        gfx::quad(c + d0 * r, c + d1 * r, c + d1 * far, c + d0 * far, BLACK);
    }
}
