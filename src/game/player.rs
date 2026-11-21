use super::bow::{self, Arrow};
use super::flicker::{Flicker, FlickerKind};
use super::gfx::{self, INK};
use super::world::{self, View};
use macroquad::prelude::*;
use std::cell::OnceCell;
use std::f32::consts::{PI, TAU};

/// How long the ghostly spawn-in lasts (FR8).
pub const SPAWN_FLICKER_S: f32 = 2.0;
const JUMP_S: f32 = 0.45;
const JUMP_HEIGHT: f32 = 22.0;
const ROLL_S: f32 = 0.4;
/// Holding Shift multiplies walking speed by this. It stacks with the
/// Speed Boost upgrade.
const SPRINT_MULTIPLIER: f32 = 1.6;
/// Raises the bow and shoots (FR19).
pub const SHOOT_KEY: KeyCode = KeyCode::F;
/// How long the stun's stars take to pop and fade once it ends (FR12).
const STUN_POP_S: f32 = 0.35;
/// How long the "item get" pose holds (FR9).
pub const CHEER_S: f32 = 1.2;

/// The sprite's box in local units, with the feet at (`SPRITE_OX`,
/// `SPRITE_OY`): wide enough for the hat brim, tall enough for the crown.
const SPRITE_W: f32 = 72.0;
const SPRITE_H: f32 = 78.0;
const SPRITE_OX: f32 = 36.0;
const SPRITE_OY: f32 = 70.0;
/// The render target is drawn at this multiple of screen size, so the
/// rotated / scaled blit stays crisp.
const SPRITE_RES: f32 = 2.0;
/// Height of the body's center above the feet: what a roll spins around.
const BODY_CENTER: f32 = 24.0;

/// Cosmetic moves (FR5). They change only how the player is drawn, never
/// position, speed or anything gameplay reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    None,
    Jump {
        t: f32,
    },
    /// `dir` is +1 rolling right, -1 rolling left.
    Roll {
        t: f32,
        dir: f32,
    },
    /// Raising the bow and loosing an arrow (FR19), facing `dir`.
    /// `volley` is false in a battle, whose view draws its own arrow.
    Shoot {
        t: f32,
        dir: f32,
        volley: bool,
    },
}

pub struct Player {
    pub pos: Vec2,
    pub base_speed: f32,
    /// +1.0 facing right, -1.0 facing left — last nonzero horizontal input,
    /// used to point the eyes so the sprite isn't direction-blind.
    pub facing: f32,
    /// Advances with distance actually covered (after the path clamp), so
    /// the walk cycle stops when the player is pressed against the railing.
    walk_phase: f32,
    moving: bool,
    pub action: Action,
    /// Spawn ghost (FR8) or hurt blink (FR11); `None` once finished.
    pub flicker: Option<Flicker>,
    /// Seconds since the player died (FR14), driving the flop-and-fade.
    pub dead: Option<f32>,
    /// Seconds of stun left (FR12): halo of stars, dizzy eyes, can't move.
    stun: f32,
    /// Counts down the stars' pop once the stun ends.
    stun_pop: f32,
    /// Free-running clock for the halo's orbit and the dizzy wobble.
    anim_t: f32,
    /// "Item get" pose time left (FR9): arms up, holding the reward.
    cheer: f32,
    /// A Gull Bandit has the hat (FR10) until it's beaten.
    pub hat_stolen: bool,
    /// Arrows loosed on the road (FR19), oldest first. Cosmetic.
    pub arrows: Vec<Arrow>,
    /// Off-screen canvas the sprite is drawn into when it has to be
    /// rotated, squashed or made translucent as one piece.
    canvas: OnceCell<RenderTarget>,
}

impl Player {
    pub fn new(start: Vec2) -> Self {
        Self {
            pos: start,
            base_speed: 140.0,
            facing: 1.0,
            walk_phase: 0.0,
            moving: false,
            action: Action::None,
            flicker: Some(Flicker::new(FlickerKind::Ghost, SPAWN_FLICKER_S)),
            canvas: OnceCell::new(),
            dead: None,
            stun: 0.0,
            stun_pop: 0.0,
            anim_t: 0.0,
            cheer: 0.0,
            hat_stolen: false,
            arrows: Vec::new(),
        }
    }

    /// Raises the bow and looses an arrow the way the player faces (F).
    pub fn shoot(&mut self) {
        self.action = Action::Shoot {
            t: 0.0,
            dir: self.facing,
            volley: true,
        };
    }

    /// A battle's attack: turns to the monster on the right and shoots,
    /// leaving the arrow's flight to the battle view.
    pub fn shoot_at_monster(&mut self) {
        self.facing = 1.0;
        self.action = Action::Shoot {
            t: 0.0,
            dir: 1.0,
            volley: false,
        };
    }

    /// The direction the bow points while a shot plays.
    fn aiming(&self) -> Option<(f32, f32)> {
        match self.action {
            Action::Shoot { t, dir, .. } => Some((t, dir)),
            _ => None,
        }
    }

    /// Stuns the player for `secs` (FR12), or extends a running stun.
    pub fn stun(&mut self, secs: f32) {
        self.stun = self.stun.max(secs);
        self.stun_pop = 0.0;
    }

    /// Strikes the "item get" pose: arms up, held still for `CHEER_S`.
    pub fn cheer(&mut self) {
        self.cheer = CHEER_S;
        self.action = Action::None;
    }

    /// Seconds into the "item get" pose, while it plays.
    pub fn cheering(&self) -> Option<f32> {
        (self.cheer > 0.0).then_some(CHEER_S - self.cheer)
    }

    pub fn stunned(&self) -> bool {
        self.stun > 0.0
    }

    /// Starts a flicker, replacing any running one.
    pub fn start_flicker(&mut self, kind: FlickerKind, duration: f32) {
        self.flicker = Some(Flicker::new(kind, duration));
    }

    /// True while the spawn-in ghost is playing: spawn protection.
    pub fn spawn_protected(&self) -> bool {
        matches!(self.flicker, Some(f) if f.kind == FlickerKind::Ghost && f.active())
    }

    /// Starts the death animation: flop over and fade.
    pub fn die(&mut self) {
        self.dead = Some(0.0);
        self.action = Action::None;
        self.flicker = None;
        self.moving = false;
    }

    /// Advances the death animation; nothing else moves while dead.
    pub fn tick_death(&mut self, dt: f32) {
        if let Some(t) = &mut self.dead {
            *t += dt;
        }
    }

    /// Advances the cosmetic timers (actions, flicker). Runs whether or not
    /// the player can move, so a jump started before a flask opened still
    /// lands.
    fn tick_effects(&mut self, dt: f32) {
        self.anim_t += dt;
        self.cheer = (self.cheer - dt).max(0.0);
        if self.stun > 0.0 {
            self.stun -= dt;
            if self.stun <= 0.0 {
                self.stun = 0.0;
                self.stun_pop = STUN_POP_S;
            }
        } else {
            self.stun_pop = (self.stun_pop - dt).max(0.0);
        }
        if let Action::Shoot {
            t,
            dir,
            volley: true,
        } = self.action
        {
            if t < bow::RELEASE_S && t + dt >= bow::RELEASE_S {
                self.loose_arrow(dir);
            }
        }
        self.action = match self.action {
            Action::Jump { t } if t + dt < JUMP_S => Action::Jump { t: t + dt },
            Action::Roll { t, dir } if t + dt < ROLL_S => Action::Roll { t: t + dt, dir },
            Action::Shoot { t, dir, volley } if t + dt < bow::SHOOT_S => Action::Shoot {
                t: t + dt,
                dir,
                volley,
            },
            _ => Action::None,
        };
        for a in &mut self.arrows {
            a.update(dt);
        }
        self.arrows.retain(|a| !a.done());
        if let Some(f) = &mut self.flicker {
            f.update(dt);
            if !f.active() {
                self.flicker = None;
            }
        }
    }

    /// Sends an arrow off from the bow hand.
    fn loose_arrow(&mut self, dir: f32) {
        if self.arrows.len() >= bow::MAX_ARROWS {
            self.arrows.remove(0);
        }
        let off = bow::NOCK_FROM_FOOT;
        self.arrows.push(Arrow::new(
            self.pos + vec2(off.x * dir, 0.0),
            world::ground_lift(self.pos.y) - off.y,
            dir,
        ));
    }

    /// W/Up walks north along the road, A/D steer across it, S backs
    /// up. `constrain` keeps the result on the walkable road.
    pub fn update(&mut self, dt: f32, speed_multiplier: f32, constrain: impl Fn(Vec2) -> Vec2) {
        self.tick_effects(dt);
        if self.stunned() || self.cheer > 0.0 {
            // Too dizzy (or too busy celebrating) to walk, jump or roll.
            self.moving = false;
            return;
        }
        if self.action == Action::None {
            if is_key_pressed(KeyCode::Space) {
                self.action = Action::Jump { t: 0.0 };
            } else if is_key_pressed(KeyCode::LeftControl) || is_key_pressed(KeyCode::RightControl)
            {
                self.action = Action::Roll {
                    t: 0.0,
                    dir: self.facing,
                };
            } else if is_key_pressed(SHOOT_KEY) {
                self.shoot();
            }
        }

        let mut dir = Vec2::ZERO;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            dir.y -= 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            dir.y += 1.0;
        }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            dir.x -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            dir.x += 1.0;
        }

        // Mid-shot the archer keeps aiming the same way, even strafing.
        if dir.x != 0.0 && self.aiming().is_none() {
            self.facing = dir.x.signum();
        }

        let sprint = if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
            SPRINT_MULTIPLIER
        } else {
            1.0
        };
        let before = self.pos;
        if dir != Vec2::ZERO {
            self.pos += dir.normalize() * self.base_speed * speed_multiplier * sprint * dt;
        }
        self.pos = constrain(self.pos);

        let moved = self.pos.distance(before);
        self.moving = moved > 0.01;
        self.walk_phase += moved * 0.16;
    }

    /// Freezes the walk cycle while the player is held in place (e.g. a
    /// task panel is open).
    pub fn stop(&mut self, dt: f32) {
        self.moving = false;
        self.tick_effects(dt);
    }

    /// Height off the ground, horizontal/vertical scale and rotation
    /// (radians) for the current action.
    fn pose(&self) -> (f32, f32, f32, f32) {
        if let Some(t) = self.dead {
            // A little hop, then a flop onto the side away from facing.
            let u = (t / 0.45).min(1.0);
            let hop = (u * PI).sin() * 10.0;
            return (hop, 1.0, 1.0, -self.facing * u * u * PI * 0.5);
        }
        let wobble = if self.stunned() {
            (self.anim_t * 5.0).sin() * 0.12
        } else {
            0.0
        };
        let (hop, sx, sy, rot) = self.action_pose();
        (hop, sx, sy, rot + wobble)
    }

    fn action_pose(&self) -> (f32, f32, f32, f32) {
        match self.action {
            Action::None | Action::Shoot { .. } => (0.0, 1.0, 1.0, 0.0),
            Action::Jump { t } => {
                let u = t / JUMP_S;
                let hop = (u * PI).sin() * JUMP_HEIGHT;
                // Squash on takeoff and landing, stretch at the top.
                let edge = (1.0 - (u * 2.0 - 1.0).abs() * 1.0).clamp(0.0, 1.0);
                let squash = if u < 0.18 || u > 0.82 {
                    -0.14
                } else {
                    0.08 * edge
                };
                (hop, 1.0 - squash, 1.0 + squash, 0.0)
            }
            Action::Roll { t, dir } => {
                let u = t / ROLL_S;
                // Ease in/out so the tumble starts and ends cleanly.
                let eased = u * u * (3.0 - 2.0 * u);
                let hop = (u * PI).sin() * 6.0;
                let tuck = 1.0 - (u * PI).sin() * 0.18;
                (hop, 1.0, tuck, eased * TAU * dir)
            }
        }
    }

    /// Chibi beachgoer: straw sun hat, Hawaiian shirt, swim shorts.
    /// `lift` is the height of whatever they're standing on.
    /// `reduce_flashing` is FR7's accessibility switch.
    pub fn draw(&self, view: &View, lift: f32, reduce_flashing: bool) {
        self.draw_at(
            view.to_screen(self.pos) - vec2(0.0, lift),
            1.0,
            reduce_flashing,
        );
    }

    /// Draws the player with their feet at screen point `ground`, `scale`
    /// times normal size (the battle view draws them bigger).
    pub fn draw_at(&self, ground: Vec2, scale: f32, reduce_flashing: bool) {
        let (hop, sx, sy, rot) = self.pose();
        let hop = hop * scale;
        let mut look = self
            .flicker
            .map(|f| f.look(reduce_flashing))
            .unwrap_or(super::flicker::Look { tint: WHITE });
        if let Some(t) = self.dead {
            look.tint.a *= 1.0 - ((t - 0.6) / 0.8).clamp(0.0, 0.7);
        }

        // The shadow stays on the ground and shrinks as the player rises.
        let shrink = 1.0 - (hop / JUMP_HEIGHT).clamp(0.0, 1.0) * 0.45;
        gfx::ellipse(
            ground + vec2(0.0, 2.0 * scale),
            13.0 * shrink * scale,
            5.5 * shrink * scale,
            Color::new(0.0, 0.0, 0.0, 0.28 * shrink * look.tint.a.max(0.4)),
        );

        let foot = ground - vec2(0.0, hop);
        let plain = rot == 0.0 && sx == 1.0 && sy == 1.0 && scale == 1.0 && look.tint == WHITE;
        if plain {
            self.draw_sprite(foot);
            return;
        }

        // Draw the sprite once into the canvas, then blit it as one piece,
        // so translucency doesn't show the overlapping parts as seams.
        let canvas = self.canvas.get_or_init(|| {
            let rt = render_target(
                (SPRITE_W * SPRITE_RES) as u32,
                (SPRITE_H * SPRITE_RES) as u32,
            );
            rt.texture.set_filter(FilterMode::Linear);
            rt
        });
        let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, SPRITE_W, SPRITE_H));
        cam.render_target = Some(canvas.clone());
        set_camera(&cam);
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
        self.draw_sprite(vec2(SPRITE_OX, SPRITE_OY));
        set_default_camera();

        let (sx, sy) = (sx * scale, sy * scale);
        let size = vec2(SPRITE_W * sx, SPRITE_H * sy);
        draw_texture_ex(
            &canvas.texture,
            foot.x - SPRITE_OX * sx,
            foot.y - SPRITE_OY * sy,
            look.tint,
            DrawTextureParams {
                dest_size: Some(size),
                rotation: rot,
                pivot: Some(foot - vec2(0.0, BODY_CENTER * sy)),
                // Render targets come back bottom-up.
                flip_y: true,
                ..Default::default()
            },
        );
    }

    /// The character itself, feet at `foot`, without the ground shadow.
    fn draw_sprite(&self, foot: Vec2) {
        let step = if self.moving {
            self.walk_phase.sin()
        } else {
            0.0
        };
        let bob = step.abs() * 2.5;

        let skin = Color::new(0.98, 0.80, 0.62, 1.0);
        let aiming = self.aiming();
        if aiming.is_none() && self.cheer <= 0.0 {
            bow::draw_slung(foot + vec2(0.0, -15.0 - bob));
        }
        let sandal = Color::new(0.95, 0.45, 0.35, 1.0);
        for (side, phase) in [(-1.0f32, step), (1.0, -step)] {
            let x = foot.x + side * 4.0;
            let lift_foot = phase.max(0.0) * 3.0;
            let hip = vec2(x, foot.y - 9.0 - bob);
            let toe = vec2(x, foot.y - 1.0 - lift_foot);
            gfx::line(hip, toe, 5.2, INK);
            gfx::line(hip, toe, 3.0, skin);
            gfx::ellipse(toe + vec2(0.0, 0.5), 3.2, 1.8, sandal);
        }

        let body = foot + vec2(0.0, -15.0 - bob);
        let shorts = Color::new(0.98, 0.48, 0.38, 1.0);
        let shirt = Color::new(0.13, 0.70, 0.72, 1.0);
        gfx::ellipse(body + vec2(0.0, 5.5), 9.5, 5.5, shorts);
        gfx::ellipse_outline(body + vec2(0.0, 5.5), 9.5, 5.5, 2.2, INK);
        gfx::ellipse(body + vec2(0.0, -1.0), 10.0, 9.0, shirt);
        gfx::ellipse_outline(body + vec2(0.0, -1.0), 10.0, 9.0, 2.4, INK);
        // A few hibiscus dots: the cheapest way to say "Hawaiian shirt".
        for (dx, dy) in [(-4.5, -3.0), (3.5, 1.5), (5.0, -5.0)] {
            draw_circle(
                body.x + dx,
                body.y + dy,
                1.6,
                Color::new(1.0, 0.95, 0.80, 1.0),
            );
        }

        // Adventurer's pouch on a strap across the shirt.
        let leather = Color::new(0.62, 0.38, 0.20, 1.0);
        gfx::line(body + vec2(-7.0, -8.0), body + vec2(6.0, 3.0), 2.6, INK);
        gfx::line(body + vec2(-7.0, -8.0), body + vec2(6.0, 3.0), 1.4, leather);
        gfx::ellipse(body + vec2(8.0, 5.0), 4.6, 3.8, leather);
        gfx::ellipse_outline(body + vec2(8.0, 5.0), 4.6, 3.8, 1.6, INK);
        draw_circle(
            body.x + 8.0,
            body.y + 4.0,
            1.1,
            Color::new(1.0, 0.85, 0.35, 1.0),
        );

        // Arms swing opposite the legs (raised overhead while cheering,
        // drawn after the hat below).
        let cheering = self.cheer > 0.0;
        if !cheering && aiming.is_none() {
            for side in [-1.0f32, 1.0] {
                let hand = body + vec2(side * 10.5, 1.0 + side * step * 2.0);
                draw_circle(hand.x, hand.y, 3.8, INK);
                draw_circle(hand.x, hand.y, 2.8, skin);
            }
        }

        let head = vec2(foot.x, foot.y - 31.0 - bob);
        let halo = head + vec2(0.0, -17.0);
        // Stars behind the hat first, the front ones after it.
        self.draw_halo(halo, false);
        draw_circle(head.x, head.y, 13.0, skin);
        draw_circle_lines(head.x, head.y, 13.0, 2.6, INK);

        // Blush — the small warmth detail that sells "cute" at a glance.
        let blush = Color::new(0.98, 0.55, 0.55, 0.55);
        gfx::ellipse(head + vec2(-7.0, 5.0), 2.6, 1.8, blush);
        gfx::ellipse(head + vec2(7.0, 5.0), 2.6, 1.8, blush);

        let look = self.facing * 2.0;
        // Sighting down the arrow: the eye away from the target shuts.
        let squint = aiming.is_some_and(|(t, _)| bow::aim(t).draw > 0.3);
        for dx in [-4.0f32, 4.0] {
            let eye = vec2(head.x + look + dx, head.y + 1.5);
            if squint && dx.signum() != self.facing.signum() {
                gfx::line(eye - vec2(2.2, 0.0), eye + vec2(2.2, 0.0), 1.6, INK);
            } else if self.stunned() {
                // Dizzy spiral eyes.
                let pts: Vec<Vec2> = (0..14)
                    .map(|i| {
                        let a = i as f32 * 0.75 + self.anim_t * 8.0;
                        eye + vec2(a.cos(), a.sin()) * (0.4 + i as f32 * 0.17)
                    })
                    .collect();
                gfx::polyline(&pts, 1.0, INK);
            } else if cheering {
                // Happy closed "^" eyes.
                gfx::polyline(
                    &[
                        eye + vec2(-2.2, 1.0),
                        eye + vec2(0.0, -1.4),
                        eye + vec2(2.2, 1.0),
                    ],
                    1.6,
                    INK,
                );
            } else {
                draw_circle(eye.x, eye.y, 2.6, INK);
                draw_circle(eye.x - 0.8, eye.y - 0.9, 0.9, WHITE);
            }
        }
        draw_arc(head.x + look, head.y + 3.0, 10, 3.2, 20.0, 1.3, 140.0, INK);

        if !self.hat_stolen {
            // Straw sun hat: wide brim, crown, red band.
            let straw = Color::new(0.97, 0.84, 0.48, 1.0);
            let brim = head + vec2(0.0, -8.0);
            gfx::ellipse(brim, 17.5, 5.5, straw);
            gfx::ellipse(brim + vec2(-2.0, -1.0), 14.0, 3.6, gfx::shade(straw, 1.06));
            gfx::ellipse_outline(brim, 17.5, 5.5, 2.2, INK);
            let crown = head + vec2(0.0, -12.0);
            gfx::ellipse(crown, 9.5, 6.5, straw);
            gfx::ellipse(
                crown + vec2(0.0, 3.2),
                9.5,
                2.2,
                Color::new(0.90, 0.30, 0.30, 1.0),
            );
            gfx::ellipse_outline(crown, 9.5, 6.5, 2.0, INK);
        }
        self.draw_halo(halo, true);

        if let Some((t, dir)) = aiming {
            // Held out in front of everything, so the bow reads at a glance.
            let aim = bow::aim(t);
            let rest = |side: f32| body + vec2(side * 10.5, 1.0);
            let grip = rest(dir).lerp(body + vec2(dir * 18.0, -3.0), aim.raise);
            bow::draw_held(grip, dir, &aim);
            let pull = if aim.nocked {
                bow::nock_point(grip, dir, &aim)
            } else {
                // The string hand stays back after the release, then drops.
                rest(-dir).lerp(grip - vec2(dir * 16.0, 0.0), aim.raise)
            };
            for hand in [grip, pull] {
                draw_circle(hand.x, hand.y, 3.8, INK);
                draw_circle(hand.x, hand.y, 2.8, skin);
            }
        }

        if cheering {
            // Both hands up, holding the reward overhead.
            for side in [-1.0f32, 1.0] {
                let shoulder = body + vec2(side * 8.0, -5.0);
                let hand = head + vec2(side * 11.0, -24.0);
                gfx::line(shoulder, hand, 4.6, INK);
                gfx::line(shoulder, hand, 3.0, skin);
                draw_circle(hand.x, hand.y, 3.8, INK);
                draw_circle(hand.x, hand.y, 2.8, skin);
            }
        }
    }

    /// The halo of stars circling the head while stunned (FR12): a tilted
    /// orbit, with the stars behind the head smaller and dimmer. `front`
    /// picks which half to draw. Once the stun ends the stars pop outward
    /// and fade.
    fn draw_halo(&self, c: Vec2, front: bool) {
        let popping = !self.stunned() && self.stun_pop > 0.0;
        if !self.stunned() && !popping {
            return;
        }
        let pop = if popping {
            1.0 - self.stun_pop / STUN_POP_S
        } else {
            0.0
        };
        let gold = Color::new(1.0, 0.86, 0.25, 1.0);
        for i in 0..4 {
            let a = self.anim_t * 4.0 + i as f32 * TAU / 4.0;
            let depth = a.sin(); // -1 behind the head, +1 in front
            if (depth >= 0.0) != front {
                continue;
            }
            let spread = 1.0 + pop * 0.9;
            let p = c + vec2(a.cos() * 15.0 * spread, depth * 4.0 * spread - pop * 6.0);
            let near = 0.75 + 0.25 * depth;
            let size = 4.2 * near * (1.0 + pop * 0.6);
            let alpha = (0.6 + 0.4 * near) * (1.0 - pop);
            gfx::star(
                p,
                size + 1.2,
                size * 0.45 + 0.8,
                5,
                a,
                1.0,
                gfx::with_alpha(INK, alpha),
            );
            gfx::star(
                p,
                size,
                size * 0.45,
                5,
                a,
                1.0,
                gfx::with_alpha(gold, alpha),
            );
        }
    }
}
