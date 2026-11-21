//! A reusable flicker: the ghostly spawn-in (FR8) and the hurt blink
//! (FR11). It only decides *how* to look each frame (alpha and tint); the
//! owner draws its sprite with that look.

use macroquad::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlickerKind {
    /// Pale and translucent, blinking fast then slowing until it settles
    /// into the solid sprite.
    Ghost,
    /// Fast red blink while the player is hurt.
    Damage,
}

#[derive(Clone, Copy, Debug)]
pub struct Flicker {
    pub kind: FlickerKind,
    pub duration: f32,
    elapsed: f32,
}

/// How to draw the sprite this frame. `tint` multiplies the sprite color;
/// its alpha is the sprite's opacity.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub tint: Color,
}

impl Flicker {
    pub fn new(kind: FlickerKind, duration: f32) -> Self {
        Self {
            kind,
            duration,
            elapsed: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.elapsed = (self.elapsed + dt).min(self.duration);
    }

    pub fn active(&self) -> bool {
        self.elapsed < self.duration
    }

    /// 0 at the start, 1 once finished.
    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0 {
            1.0
        } else {
            self.elapsed / self.duration
        }
    }

    /// Blink phase in cycles. The rate glides from `f0` to `f1` Hz over the
    /// flicker, so a ghost's blink visibly slows down as it settles.
    fn phase(&self, f0: f32, f1: f32) -> f32 {
        let t = self.elapsed;
        f0 * t - (f0 - f1) * t * t / (2.0 * self.duration.max(0.001))
    }

    /// The look for this frame. With `reduce_flashing` there is no
    /// blinking: a steady fade (ghost) or a steady tint (damage).
    pub fn look(&self, reduce_flashing: bool) -> Look {
        let p = self.progress();
        if !self.active() {
            return Look { tint: WHITE };
        }
        match self.kind {
            FlickerKind::Ghost => {
                let pale = Color::new(0.72, 0.86, 1.0, 1.0);
                let steady = 0.30 + 0.70 * p;
                let alpha = if reduce_flashing {
                    steady
                } else {
                    let on = (self.phase(14.0, 2.0) * std::f32::consts::TAU).sin() > 0.0;
                    // The "off" frames rise towards solid so the blink fades
                    // out instead of stopping abruptly.
                    if on {
                        0.35 + 0.65 * p
                    } else {
                        0.10 + 0.90 * p * p
                    }
                };
                let k = 1.0 - p;
                Look {
                    tint: Color::new(
                        1.0 + (pale.r - 1.0) * k,
                        1.0 + (pale.g - 1.0) * k,
                        1.0 + (pale.b - 1.0) * k,
                        alpha,
                    ),
                }
            }
            FlickerKind::Damage => {
                let hurt = Color::new(1.0, 0.42, 0.42, 1.0);
                if self.elapsed < 0.1 {
                    // The hit itself: one hard red frame before the blink.
                    Look {
                        tint: Color::new(1.0, 0.25, 0.25, 1.0),
                    }
                } else if reduce_flashing {
                    Look { tint: hurt }
                } else {
                    let on = (self.phase(12.0, 12.0) * std::f32::consts::TAU).sin() > 0.0;
                    Look {
                        tint: if on {
                            hurt
                        } else {
                            Color::new(1.0, 1.0, 1.0, 0.45)
                        },
                    }
                }
            }
        }
    }
}
