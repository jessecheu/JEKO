//! Beach scenery: what gets scattered along each chunk of the boardwalk,
//! and how each prop is drawn.
//!
//! Generation works in zones that are defined relative to the path and the
//! coastline at each `y` (open water, the wet sand at the waterline, the
//! beach between the water and the boardwalk, the strip right of the
//! boardwalk, and the dunes), so props follow the path as it winds and
//! never land in the surf or on the deck.

use super::gfx::{self, INK};
use super::world::{self, View, PATH_HALF_W, PX_PER_METER};
use ::rand::rngs::StdRng;
use ::rand::{Rng, SeedableRng};
use macroquad::prelude::*;
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy)]
pub enum Kind {
    // Flat: painted with the ground, under the player and every tall prop.
    Speckle {
        size: f32,
        dark: bool,
    },
    Ripple {
        len: f32,
    },
    Shell {
        rot: f32,
        tint: Color,
    },
    Starfish {
        rot: f32,
        tint: Color,
    },
    Towel {
        rot: f32,
        base: Color,
        stripe: Color,
    },
    Flipflops {
        rot: f32,
        color: Color,
    },
    /// A trail of footprints heading off at `angle` radians.
    Footprints {
        angle: f32,
    },
    Coconut,
    GrassTuft {
        size: f32,
        phase: f32,
    },
    WaveCrest {
        phase: f32,
    },
    Glint {
        phase: f32,
    },

    // Tall: depth-sorted against the player by their base `y`.
    Palm {
        height: f32,
        lean: f32,
        phase: f32,
    },
    Umbrella {
        canopy: Color,
        stripe: Color,
        tilt: f32,
    },
    Lounger {
        fabric: Color,
    },
    Cooler {
        color: Color,
    },
    Sandcastle,
    Bucket {
        color: Color,
    },
    Surfboard {
        color: Color,
        stripe: Color,
        tilt: f32,
    },
    BeachBall {
        phase: f32,
    },
    Crab {
        phase: f32,
    },
    Rock {
        size: f32,
        wet: bool,
    },
    Buoy {
        phase: f32,
    },
    Sailboat {
        phase: f32,
        sail: Color,
    },
    SwimRing {
        phase: f32,
        color: Color,
    },
    Hut {
        flip: bool,
    },
    TikiTorch {
        phase: f32,
    },
    Hibiscus {
        flower: Color,
    },
    Lifeguard,
    Marker {
        meters: u32,
    },
    Signpost,
    /// Professor Pebble, the hub's tutorial NPC (FR9).
    Pebble,
    /// The arch over the boardwalk's south end: the hub's gateway onto the
    /// road (FR15).
    Gateway,
}

pub struct Decoration {
    pub pos: Vec2,
    pub kind: Kind,
}

const CORAL: Color = Color::new(0.98, 0.45, 0.40, 1.0);
const SUNNY: Color = Color::new(1.00, 0.82, 0.28, 1.0);
const TEAL: Color = Color::new(0.18, 0.72, 0.74, 1.0);
const SKY: Color = Color::new(0.35, 0.62, 0.95, 1.0);
const PINK: Color = Color::new(0.98, 0.55, 0.70, 1.0);
const CREAM: Color = Color::new(1.00, 0.97, 0.90, 1.0);
const SAND_FRONT: Color = Color::new(0.86, 0.70, 0.45, 1.0);
const SAND_TOP: Color = Color::new(0.98, 0.87, 0.63, 1.0);
const WOOD: Color = Color::new(0.62, 0.43, 0.25, 1.0);
const STRAW: Color = Color::new(0.90, 0.74, 0.40, 1.0);
const WATER_MID: Color = Color::new(0.12, 0.60, 0.80, 1.0);
const PALETTE: [Color; 5] = [CORAL, SUNNY, TEAL, SKY, PINK];

#[derive(Clone, Copy)]
enum Zone {
    /// Water well past the break line, within the default 960px view —
    /// boats, buoys, rocks.
    Ocean,
    /// Water left of the default view, only seen in a wider window.
    FarOcean,
    /// Any on-screen water — wave crests and glints.
    Sea,
    FarSea,
    /// Just inside the swash — swim rings.
    Shallows,
    /// The band of wet sand the waves wash over.
    Waterline,
    /// Dry sand between the water and the boardwalk.
    Beach,
    /// Dry sand between the boardwalk and the dunes.
    Inland,
    /// Dunes within the default view.
    Dunes,
    /// Dunes right of the default view, only seen in a wider window.
    FarDunes,
}

/// Valid x range at height `y` for something of footprint radius `r`.
/// Most props go in the zones the default 960px window actually shows;
/// the `Far*` zones get a lighter sprinkling so a maximized window
/// doesn't reveal bare sand and empty sea at the edges.
fn zone_range(zone: Zone, y: f32, r: f32) -> (f32, f32) {
    let c = world::path_center(y);
    let shore = world::shore_x(y);
    let dune = world::dune_x(y);
    let far = 420.0;
    match zone {
        Zone::Ocean => (r - 10.0, shore - 80.0 - r),
        Zone::FarOcean => (-far, -10.0 - r),
        Zone::Sea => (-10.0, shore - 30.0),
        Zone::FarSea => (-far, -10.0),
        Zone::Shallows => (shore - 60.0, shore - 35.0),
        Zone::Waterline => (shore + 10.0, shore + 26.0),
        Zone::Beach => (shore + 38.0 + r, c - PATH_HALF_W - 22.0 - r),
        Zone::Inland => (c + PATH_HALF_W + 26.0 + r, dune - 6.0 - r),
        Zone::Dunes => (dune + 10.0 + r, world::WORLD_W - 6.0),
        Zone::FarDunes => (world::WORLD_W + r, world::WORLD_W + far),
    }
}

struct Gen {
    rng: StdRng,
    north: f32,
    south: f32,
    /// Footprints of props already placed in this chunk, so big things
    /// don't overlap. Small flat clutter skips this check.
    taken: Vec<(Vec2, f32)>,
    out: Vec<Decoration>,
}

impl Gen {
    /// A free spot for a prop of footprint radius `r`, or `None` if the
    /// zone is too crowded or too narrow here. Kept `r` clear of the chunk
    /// edges, since the overlap check can't see the neighboring chunks.
    fn place(&mut self, zone: Zone, r: f32) -> Option<Vec2> {
        let (north, south) = (self.north + r, self.south - r);
        if south <= north {
            return None;
        }
        for _ in 0..16 {
            let y = self.rng.gen_range(north..south);
            let (lo, hi) = zone_range(zone, y, r);
            if hi <= lo {
                continue;
            }
            let p = vec2(self.rng.gen_range(lo..hi), y);
            // Vertical gaps count for less: props stick up off the ground,
            // so two stacked north/south need more room than side by side.
            let clear = |&(q, qr): &(Vec2, f32)| ((p - q) * vec2(1.0, 0.7)).length() >= qr + r;
            if self.taken.iter().all(clear) {
                self.taken.push((p, r));
                return Some(p);
            }
        }
        None
    }

    fn scatter(&mut self, zone: Zone) -> Option<Vec2> {
        for _ in 0..4 {
            let y = self.rng.gen_range(self.north..self.south);
            let (lo, hi) = zone_range(zone, y, 0.0);
            if hi > lo {
                return Some(vec2(self.rng.gen_range(lo..hi), y));
            }
        }
        None
    }

    fn chance(&mut self, p: f64) -> bool {
        self.rng.gen_bool(p)
    }

    fn pick(&mut self) -> Color {
        PALETTE[self.rng.gen_range(0..PALETTE.len())]
    }

    fn phase(&mut self) -> f32 {
        self.rng.gen_range(0.0..TAU)
    }

    fn push(&mut self, pos: Vec2, kind: Kind) {
        self.out.push(Decoration { pos, kind });
    }

    fn add(&mut self, zone: Zone, r: f32, kind: impl FnOnce(&mut Self) -> Kind) -> Option<Vec2> {
        let p = self.place(zone, r)?;
        let k = kind(self);
        self.push(p, k);
        Some(p)
    }

    fn maybe(&mut self, p: f64, zone: Zone, r: f32, kind: impl FnOnce(&mut Self) -> Kind) {
        if self.chance(p) {
            self.add(zone, r, kind);
        }
    }

    fn sprinkle(&mut self, zone: Zone, n: usize, kind: impl Fn(&mut Self) -> Kind) {
        for _ in 0..n {
            if let Some(p) = self.scatter(zone) {
                let k = kind(self);
                self.push(p, k);
            }
        }
    }

    fn palm(&mut self, zone: Zone, min_h: f32, max_h: f32, lean: (f32, f32)) {
        let placed = self.add(zone, 30.0, |g| Kind::Palm {
            height: g.rng.gen_range(min_h..max_h),
            lean: g.rng.gen_range(lean.0..lean.1),
            phase: g.phase(),
        });
        // A fallen coconut or two at the foot of some palms.
        if let Some(p) = placed {
            let n = self.rng.gen_range(0..3);
            for _ in 0..n {
                let off = vec2(
                    self.rng.gen_range(-16.0..16.0),
                    self.rng.gen_range(3.0..12.0),
                );
                self.push(p + off, Kind::Coconut);
            }
        }
    }

    fn speckles(&mut self, zone: Zone, n: usize, dark_bias: f64) {
        self.sprinkle(zone, n, |g| Kind::Speckle {
            size: g.rng.gen_range(1.0..2.4),
            dark: g.chance(dark_bias),
        });
    }
}

fn shell(g: &mut Gen) -> Kind {
    let tints = [CREAM, PINK, Color::new(0.98, 0.80, 0.62, 1.0)];
    Kind::Shell {
        rot: g.rng.gen_range(-0.6..0.6),
        tint: tints[g.rng.gen_range(0..tints.len())],
    }
}

fn starfish(g: &mut Gen) -> Kind {
    Kind::Starfish {
        rot: g.phase(),
        tint: if g.chance(0.5) {
            CORAL
        } else {
            Color::new(1.0, 0.62, 0.30, 1.0)
        },
    }
}

fn surfboard(g: &mut Gen) -> Kind {
    Kind::Surfboard {
        color: g.pick(),
        stripe: CREAM,
        tilt: g.rng.gen_range(-0.25..0.25),
    }
}

/// All scenery for chunk `k`. `spot` is where this chunk's bottle sits (if
/// it has one) so it can be framed by a pair of tiki torches.
/// The spawn hub (FR15): a fixed layout south of the gateway, so every run
/// starts in the same place. `(position, keep-clear radius, kind)`.
fn hub_layout() -> Vec<(Vec2, f32, Kind)> {
    let gate = world::BOARDWALK_SOUTH_END;
    let c = world::path_center(gate);
    let at = |dx: f32, dy: f32| vec2(c + dx, gate + dy);
    let mut v = vec![
        (at(0.0, 2.0), 0.0, Kind::Gateway),
        // Torches line the funnel up to the gateway.
        (at(-112.0, 70.0), 14.0, Kind::TikiTorch { phase: 0.3 }),
        (at(112.0, 70.0), 14.0, Kind::TikiTorch { phase: 2.1 }),
        (at(-168.0, 150.0), 14.0, Kind::TikiTorch { phase: 4.0 }),
        (at(168.0, 150.0), 14.0, Kind::TikiTorch { phase: 5.2 }),
        // The lifeguard tower watches the hub from the waterline side.
        (at(-168.0, 290.0), 46.0, Kind::Lifeguard),
        (
            at(-118.0, 262.0),
            16.0,
            Kind::Surfboard {
                color: SKY,
                stripe: CREAM,
                tilt: -0.15,
            },
        ),
        (
            at(-98.0, 268.0),
            16.0,
            Kind::Surfboard {
                color: CORAL,
                stripe: CREAM,
                tilt: 0.12,
            },
        ),
        // Snack shack on the dune side.
        (at(212.0, 330.0), 50.0, Kind::Hut { flip: true }),
        (at(160.0, 360.0), 16.0, Kind::Cooler { color: TEAL }),
        // A sunbathing corner and a stray beach ball.
        (
            at(-130.0, 500.0),
            32.0,
            Kind::Umbrella {
                canopy: CORAL,
                stripe: CREAM,
                tilt: 0.1,
            },
        ),
        (at(-96.0, 528.0), 22.0, Kind::Lounger { fabric: TEAL }),
        (
            at(150.0, 560.0),
            32.0,
            Kind::Umbrella {
                canopy: SUNNY,
                stripe: TEAL,
                tilt: -0.1,
            },
        ),
        (at(72.0, 470.0), 14.0, Kind::BeachBall { phase: 1.0 }),
        (
            at(250.0, 470.0),
            30.0,
            Kind::Palm {
                height: 96.0,
                lean: -0.2,
                phase: 1.4,
            },
        ),
        (
            at(-200.0, 680.0),
            30.0,
            Kind::Palm {
                height: 104.0,
                lean: 0.25,
                phase: 3.0,
            },
        ),
        (at(30.0, 690.0), 26.0, Kind::Sandcastle),
        (world::pebble_pos(), 26.0, Kind::Pebble),
        // Welcome sign beside the spawn point.
        (at(112.0, 236.0), 48.0, Kind::Signpost),
    ];
    // Keep a clear walk from the spawn point up to the gateway.
    v.push((
        at(0.0, world::PLAYER_START_Y - gate),
        40.0,
        Kind::Speckle {
            size: 1.0,
            dark: false,
        },
    ));
    v
}

pub fn generate_chunk(k: i64, spot: Option<Vec2>) -> Vec<Decoration> {
    let (north, south) = world::chunk_span(k);
    let seed = 0xBEAC_4000_u64 ^ (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut g = Gen {
        rng: StdRng::seed_from_u64(seed),
        north,
        south,
        taken: Vec::new(),
        out: Vec::new(),
    };

    // The spawn hub's hand-placed landmarks. Reserved before anything
    // random, so the random scenery fits around them.
    for (pos, r, kind) in hub_layout() {
        if pos.y >= north && pos.y < south {
            g.taken.push((pos, r));
            g.push(pos, kind);
        }
    }

    // Flat texture first so everything else paints over it.
    g.speckles(Zone::Beach, 26, 0.6);
    g.speckles(Zone::Inland, 16, 0.6);
    g.speckles(Zone::Dunes, 18, 0.7);
    g.speckles(Zone::FarDunes, 14, 0.7);
    g.sprinkle(Zone::Beach, 3, |g| Kind::Ripple {
        len: g.rng.gen_range(18.0..34.0),
    });
    g.sprinkle(Zone::Inland, 2, |g| Kind::Ripple {
        len: g.rng.gen_range(18.0..30.0),
    });
    g.sprinkle(Zone::Sea, 7, |g| Kind::WaveCrest { phase: g.phase() });
    g.sprinkle(Zone::Sea, 8, |g| Kind::Glint { phase: g.phase() });
    g.sprinkle(Zone::FarSea, 6, |g| Kind::WaveCrest { phase: g.phase() });
    g.sprinkle(Zone::FarSea, 6, |g| Kind::Glint { phase: g.phase() });
    g.sprinkle(Zone::Waterline, 2, shell);
    g.sprinkle(Zone::Beach, 1, shell);
    g.sprinkle(Zone::Inland, 1, shell);
    if g.chance(0.6) {
        g.sprinkle(Zone::Waterline, 1, starfish);
    }
    if g.chance(0.35) {
        g.sprinkle(Zone::Beach, 1, starfish);
    }
    // Someone wandered down to the water.
    if g.chance(0.4) {
        g.sprinkle(Zone::Beach, 1, |g| Kind::Footprints {
            angle: PI + g.rng.gen_range(-0.5..0.5),
        });
    }
    if g.chance(0.25) {
        g.sprinkle(Zone::Inland, 1, |g| Kind::Footprints {
            angle: g.rng.gen_range(-0.5..0.5),
        });
    }
    g.sprinkle(Zone::Dunes, 16, |g| Kind::GrassTuft {
        size: g.rng.gen_range(10.0..18.0),
        phase: g.phase(),
    });
    g.sprinkle(Zone::FarDunes, 8, |g| Kind::GrassTuft {
        size: g.rng.gen_range(10.0..18.0),
        phase: g.phase(),
    });

    // Path-side props first: they're pinned to exact spots, so they claim
    // their footprint before random placement fills in around them.
    if let Some(p) = spot {
        for side in [-1.0, 1.0] {
            let torch = vec2(world::path_center(p.y) + side * (PATH_HALF_W + 20.0), p.y);
            g.taken.push((torch, 14.0));
            let phase = g.phase();
            g.push(torch, Kind::TikiTorch { phase });
        }
    }
    if g.chance(0.35) {
        let y = g.rng.gen_range(north..south);
        let torch = vec2(world::path_center(y) + PATH_HALF_W + 20.0, y);
        if g.taken.iter().all(|&(q, r)| q.distance(torch) > r + 14.0) {
            g.taken.push((torch, 14.0));
            let phase = g.phase();
            g.push(torch, Kind::TikiTorch { phase });
        }
    }
    // Trail markers every 50 m on the ocean side of the deck.
    let first_m = ((world::ROAD_START_Y - south) / PX_PER_METER / 50.0)
        .ceil()
        .max(1.0) as u32;
    let mut m = first_m * 50;
    loop {
        let y = world::ROAD_START_Y - m as f32 * PX_PER_METER;
        if y <= north {
            break;
        }
        if y < south {
            let marker = vec2(world::path_center(y) - PATH_HALF_W - 20.0, y);
            g.taken.push((marker, 18.0));
            g.push(marker, Kind::Marker { meters: m });
        }
        m += 50;
    }

    // Ocean.
    g.maybe(0.40, Zone::Ocean, 34.0, |g| Kind::Sailboat {
        phase: g.phase(),
        sail: g.pick(),
    });
    g.maybe(0.45, Zone::Ocean, 14.0, |g| Kind::Buoy { phase: g.phase() });
    g.maybe(0.40, Zone::Ocean, 16.0, |g| Kind::Rock {
        size: g.rng.gen_range(9.0..15.0),
        wet: true,
    });
    g.maybe(0.30, Zone::FarOcean, 40.0, |g| Kind::Sailboat {
        phase: g.phase(),
        sail: g.pick(),
    });
    g.maybe(0.30, Zone::FarOcean, 14.0, |g| Kind::Buoy {
        phase: g.phase(),
    });
    g.maybe(0.30, Zone::Shallows, 14.0, |g| Kind::SwimRing {
        phase: g.phase(),
        color: if g.chance(0.5) { CORAL } else { SUNNY },
    });
    g.maybe(0.5, Zone::Waterline, 14.0, |g| Kind::Crab {
        phase: g.phase(),
    });

    // Beach between the water and the boardwalk: the busy, sunbathing side.
    let umbrellas = 1 + g.chance(0.5) as usize;
    for _ in 0..umbrellas {
        if let Some(p) = g.place(Zone::Beach, 34.0) {
            let (canopy, stripe) = (g.pick(), if g.chance(0.6) { CREAM } else { g.pick() });
            let towel_at = p + vec2(20.0, 6.0);
            g.taken.push((towel_at, 16.0));
            let towel = Kind::Towel {
                rot: g.rng.gen_range(-0.35..0.35),
                base: g.pick(),
                stripe: CREAM,
            };
            g.push(towel_at, towel);
            let tilt = g.rng.gen_range(-0.25..0.25);
            g.push(
                p,
                Kind::Umbrella {
                    canopy,
                    stripe,
                    tilt,
                },
            );
            if g.chance(0.4) {
                let color = g.pick();
                let rot = g.rng.gen_range(-0.4..0.4);
                g.push(p + vec2(-14.0, 14.0), Kind::Flipflops { rot, color });
            }
        }
    }
    g.maybe(0.45, Zone::Beach, 22.0, |g| Kind::Lounger {
        fabric: g.pick(),
    });
    g.maybe(0.45, Zone::Beach, 28.0, |_| Kind::Sandcastle);
    g.maybe(0.40, Zone::Beach, 14.0, |g| Kind::Bucket {
        color: g.pick(),
    });
    g.maybe(0.30, Zone::Beach, 14.0, |g| Kind::Cooler {
        color: if g.chance(0.5) { SKY } else { CORAL },
    });
    g.maybe(0.35, Zone::Beach, 14.0, surfboard);
    g.maybe(0.35, Zone::Beach, 12.0, |g| Kind::BeachBall {
        phase: g.phase(),
    });
    g.maybe(0.14, Zone::Beach, 36.0, |_| Kind::Lifeguard);
    g.maybe(0.20, Zone::Beach, 14.0, |g| Kind::Rock {
        size: g.rng.gen_range(7.0..12.0),
        wet: false,
    });
    // An occasional palm on the beach side too, so the left half isn't
    // all umbrellas.
    if g.chance(0.25) {
        g.palm(Zone::Beach, 70.0, 95.0, (-0.4, -0.1));
    }

    // Strip between the boardwalk and the dunes.
    g.maybe(0.30, Zone::Inland, 22.0, |g| Kind::Lounger {
        fabric: g.pick(),
    });
    g.maybe(0.25, Zone::Inland, 14.0, surfboard);
    g.maybe(0.25, Zone::Inland, 22.0, |g| Kind::Towel {
        rot: g.rng.gen_range(-0.4..0.4),
        base: g.pick(),
        stripe: CREAM,
    });
    g.maybe(0.20, Zone::Inland, 12.0, |g| Kind::Cooler {
        color: if g.chance(0.5) { SKY } else { CORAL },
    });
    g.maybe(0.20, Zone::Inland, 28.0, |_| Kind::Sandcastle);
    g.maybe(0.20, Zone::Inland, 12.0, |g| Kind::BeachBall {
        phase: g.phase(),
    });
    if g.chance(0.2) {
        g.sprinkle(Zone::Inland, 1, |g| Kind::Flipflops {
            rot: g.rng.gen_range(-0.5..0.5),
            color: g.pick(),
        });
    }

    // Dunes: the shady, leafy side.
    g.maybe(0.35, Zone::Dunes, 50.0, |g| Kind::Hut {
        flip: g.chance(0.5),
    });
    let palms = 2 + g.chance(0.6) as usize;
    for _ in 0..palms {
        g.palm(Zone::Dunes, 70.0, 112.0, (-0.35, 0.35));
    }
    let bushes = 1 + g.chance(0.5) as usize;
    for _ in 0..bushes {
        g.add(Zone::Dunes, 22.0, |g| Kind::Hibiscus {
            flower: if g.chance(0.5) { CORAL } else { PINK },
        });
    }
    g.maybe(0.40, Zone::Dunes, 16.0, |g| Kind::Rock {
        size: g.rng.gen_range(8.0..14.0),
        wet: false,
    });
    let far_palms = 1 + g.chance(0.5) as usize;
    for _ in 0..far_palms {
        g.palm(Zone::FarDunes, 70.0, 112.0, (-0.35, 0.35));
    }
    g.maybe(0.4, Zone::FarDunes, 22.0, |g| Kind::Hibiscus {
        flower: if g.chance(0.5) { CORAL } else { PINK },
    });
    g.maybe(0.2, Zone::FarDunes, 50.0, |g| Kind::Hut {
        flip: g.chance(0.5),
    });

    // The open sand below the start of the boardwalk.
    if south > world::BOARDWALK_SOUTH_END {
        let c = world::path_center(world::BOARDWALK_SOUTH_END);
        let plaza = world::BOARDWALK_SOUTH_END;
        g.push(vec2(c - 36.0, plaza + 70.0), Kind::Sandcastle);
        g.push(vec2(c + 4.0, plaza + 84.0), Kind::Bucket { color: CORAL });
        g.push(
            vec2(c + 52.0, plaza + 46.0),
            Kind::Starfish {
                rot: 0.4,
                tint: CORAL,
            },
        );
        g.push(
            vec2(c + 30.0, plaza + 120.0),
            Kind::Shell {
                rot: 0.2,
                tint: PINK,
            },
        );
    }

    g.out
}

impl Decoration {
    pub fn is_flat(&self) -> bool {
        matches!(
            self.kind,
            Kind::Speckle { .. }
                | Kind::Ripple { .. }
                | Kind::Shell { .. }
                | Kind::Starfish { .. }
                | Kind::Towel { .. }
                | Kind::Flipflops { .. }
                | Kind::Footprints { .. }
                | Kind::Coconut
                | Kind::GrassTuft { .. }
                | Kind::WaveCrest { .. }
                | Kind::Glint { .. }
        )
    }

    pub fn draw(&self, view: &View, t: f32) {
        let s = view.to_screen(self.pos);
        match self.kind {
            Kind::Speckle { size, dark } => {
                let c = if dark {
                    Color::new(0.78, 0.63, 0.42, 0.55)
                } else {
                    Color::new(1.0, 0.97, 0.88, 0.7)
                };
                draw_rectangle(s.x, s.y, size, size, c);
            }
            Kind::Ripple { len } => {
                for j in 0..2 {
                    let pts: Vec<Vec2> = (0..=6)
                        .map(|i| {
                            let u = i as f32 / 6.0;
                            s + vec2(
                                len * (u - 0.5) + j as f32 * 3.0,
                                j as f32 * 5.0 + 1.8 * (u * 6.5).sin(),
                            )
                        })
                        .collect();
                    gfx::polyline(&pts, 1.4, Color::new(0.82, 0.68, 0.46, 0.55));
                }
            }
            Kind::Shell { rot, tint } => draw_shell(s, rot, tint),
            Kind::Starfish { rot, tint } => {
                gfx::star(s, 9.0, 3.8, 5, rot, 0.7, gfx::shade(tint, 0.7));
                gfx::star(s + vec2(-0.5, -0.5), 7.8, 3.0, 5, rot, 0.7, tint);
                for i in 0..5 {
                    let a = rot + i as f32 / 5.0 * TAU;
                    let d = vec2(a.cos(), a.sin() * 0.7) * 4.0;
                    draw_circle(s.x + d.x, s.y + d.y, 0.9, gfx::with_alpha(CREAM, 0.8));
                }
            }
            Kind::Towel { rot, base, stripe } => draw_towel(s, rot, base, stripe),
            Kind::Flipflops { rot, color } => draw_flipflops(s, rot, color),
            Kind::Footprints { angle } => draw_footprints(s, angle),
            Kind::Coconut => {
                gfx::ellipse(s + vec2(1.5, 1.0), 4.5, 2.0, gfx::SHADOW);
                draw_circle(s.x, s.y - 2.5, 3.8, Color::new(0.42, 0.28, 0.13, 1.0));
                draw_circle(s.x - 1.2, s.y - 3.7, 1.2, Color::new(0.62, 0.45, 0.25, 1.0));
            }
            Kind::GrassTuft { size, phase } => draw_tuft(s, size, phase, t),
            Kind::WaveCrest { phase } => {
                let a = 0.55 * (t * 0.9 + phase).sin().max(0.0);
                if a > 0.02 {
                    let x = s.x + 6.0 * (t * 0.3 + phase).sin();
                    for dx in [0.0, 13.0] {
                        draw_arc(
                            x + dx,
                            s.y,
                            12,
                            7.0,
                            200.0,
                            1.6,
                            140.0,
                            gfx::with_alpha(WHITE, a),
                        );
                    }
                }
            }
            Kind::Glint { phase } => {
                let a = (t * 2.3 + phase).sin().max(0.0).powi(6);
                if a > 0.02 {
                    gfx::star(
                        s,
                        2.0 + 3.0 * a,
                        0.8,
                        4,
                        0.0,
                        1.0,
                        gfx::with_alpha(WHITE, a),
                    );
                }
            }
            Kind::Palm {
                height,
                lean,
                phase,
            } => draw_palm(s, height, lean, phase, t),
            Kind::Umbrella {
                canopy,
                stripe,
                tilt,
            } => draw_umbrella(s, canopy, stripe, tilt),
            Kind::Lounger { fabric } => draw_lounger(s, fabric),
            Kind::Cooler { color } => draw_cooler(s, color),
            Kind::Sandcastle => draw_sandcastle(s, t),
            Kind::Bucket { color } => draw_bucket(s, color),
            Kind::Surfboard {
                color,
                stripe,
                tilt,
            } => draw_surfboard(s, color, stripe, tilt),
            Kind::BeachBall { phase } => draw_beach_ball(s, phase, t),
            Kind::Crab { phase } => draw_crab(s, phase, t),
            Kind::Rock { size, wet } => draw_rock(s, size, wet, t),
            Kind::Buoy { phase } => draw_buoy(s, phase, t),
            Kind::Sailboat { phase, sail } => draw_sailboat(s, phase, sail, t),
            Kind::SwimRing { phase, color } => draw_swim_ring(s, phase, color, t),
            Kind::Hut { flip } => draw_hut(s, flip),
            Kind::TikiTorch { phase } => draw_torch(s, phase, t),
            Kind::Hibiscus { flower } => draw_hibiscus(s, flower),
            Kind::Lifeguard => draw_lifeguard(s, t),
            Kind::Marker { meters } => draw_marker(s, meters),
            Kind::Signpost => draw_signpost(s),
            Kind::Gateway => draw_gateway(s, t),
            Kind::Pebble => draw_pebble(s, t),
        }
    }
}

fn dir(a: f32) -> Vec2 {
    vec2(a.cos(), a.sin())
}

fn draw_shell(s: Vec2, rot: f32, tint: Color) {
    let r = 6.5;
    let spread = 2.4;
    let rim = |i: usize| {
        let a = rot - PI / 2.0 - spread / 2.0 + spread * i as f32 / 6.0;
        s + vec2(a.cos(), a.sin() * 0.8) * r
    };
    for i in 0..6 {
        let c = if i % 2 == 0 {
            tint
        } else {
            gfx::shade(tint, 0.88)
        };
        draw_triangle(s, rim(i), rim(i + 1), c);
    }
    for i in 1..6 {
        gfx::line(s, rim(i), 0.8, gfx::with_alpha(INK, 0.3));
    }
    let arc: Vec<Vec2> = (0..=6).map(rim).collect();
    gfx::polyline(&arc, 1.0, gfx::with_alpha(INK, 0.5));
    gfx::ellipse(s, 2.2, 1.4, gfx::shade(tint, 0.75));
}

fn draw_towel(s: Vec2, rot: f32, base: Color, stripe: Color) {
    let half = vec2(12.0, 19.0);
    let along = vec2(-rot.sin(), rot.cos());
    gfx::rot_rect(
        s + vec2(1.5, 1.5),
        half,
        rot,
        gfx::with_alpha(gfx::SHADOW, 0.12),
    );
    gfx::rot_rect(s, half, rot, base);
    for j in [-11.0, -7.0, 7.0, 11.0] {
        gfx::rot_rect(s + along * j, vec2(half.x, 1.3), rot, stripe);
    }
    // Fringe at both short ends.
    let across = vec2(rot.cos(), rot.sin());
    for end in [-1.0, 1.0] {
        for i in 0..5 {
            let x = -half.x + 2.0 + i as f32 * 5.0;
            let a = s + across * x + along * (end * half.y);
            gfx::line(a, a + along * (end * 3.0), 1.0, stripe);
        }
    }
}

fn draw_flipflops(s: Vec2, rot: f32, color: Color) {
    let deg = rot.to_degrees();
    for side in [-1.0f32, 1.0] {
        let c = s + vec2(side * 4.5, side * 1.5);
        let up = vec2((rot + side * 0.1).sin(), -(rot + side * 0.1).cos());
        gfx::ellipse_rot(c, 3.4, 7.5, deg + side * 6.0, gfx::shade(color, 0.7));
        gfx::ellipse_rot(c - vec2(0.4, 0.4), 2.9, 7.0, deg + side * 6.0, color);
        let toe = c + up * 4.0;
        for strap in [-1.0f32, 1.0] {
            let end = c + up.perp() * (strap * 2.8) - up * 0.5;
            gfx::line(toe, end, 1.1, CREAM);
        }
    }
}

fn draw_footprints(s: Vec2, angle: f32) {
    let ahead = dir(angle);
    let side = ahead.perp();
    let print = Color::new(0.72, 0.58, 0.38, 0.45);
    for i in 0..7 {
        let foot = if i % 2 == 0 { 1.0 } else { -1.0 };
        let p = s + ahead * (i as f32 * 11.0) + side * (foot * 3.2);
        gfx::ellipse_rot(p, 2.6, 1.6, angle.to_degrees(), print);
        gfx::ellipse_rot(p + ahead * 3.2, 1.2, 1.2, 0.0, print);
    }
}

/// A striped sun lounger seen side-on: feet toward the sea (west),
/// backrest raised at the east end, so its profile reads at a glance.
fn draw_lounger(s: Vec2, fabric: Color) {
    let frame = Color::new(0.97, 0.96, 0.93, 1.0);
    let legs = 6.0;
    gfx::ground_shadow(s + vec2(6.0, 2.0), 24.0, 7.0);

    let (west, east) = (s.x - 18.0, s.x + 8.0);
    let (back, front) = (s.y - 8.0 - legs, s.y - legs);
    for x in [west + 3.0, east - 2.0] {
        gfx::line(
            vec2(x, back + 1.0),
            vec2(x, back + legs + 1.0),
            1.6,
            gfx::shade(frame, 0.7),
        );
    }

    // Seat: three stripes along its length.
    let band = (front - back) / 3.0;
    for i in 0..3 {
        let col = if i == 1 { CREAM } else { fabric };
        draw_rectangle(west, back + band * i as f32, east - west, band, col);
    }
    draw_rectangle_lines(west, back, east - west, front - back, 1.3, INK);

    // Backrest: hinged on the seat's east edge, tilted up and back. Its
    // stripes run hinge-to-top, matching the seat's.
    let rise = vec2(9.0, -17.0);
    let (hinge_back, hinge_front) = (vec2(east, back), vec2(east, front));
    for i in 0..3 {
        let (u0, u1) = (i as f32 / 3.0, (i + 1) as f32 / 3.0);
        let a = hinge_back.lerp(hinge_front, u0);
        let b = hinge_back.lerp(hinge_front, u1);
        let col = if i == 1 {
            CREAM
        } else {
            gfx::shade(fabric, 1.1)
        };
        gfx::quad(a, b, b + rise, a + rise, col);
    }
    gfx::polyline(
        &[
            hinge_back,
            hinge_front,
            hinge_front + rise,
            hinge_back + rise,
            hinge_back,
        ],
        1.3,
        INK,
    );
    gfx::line(
        vec2(west, front + 1.0),
        hinge_front + vec2(0.0, 1.0),
        1.8,
        frame,
    );
    gfx::line(
        hinge_front + vec2(0.0, 1.0),
        hinge_front + rise + vec2(0.0, 1.0),
        1.8,
        frame,
    );
    for x in [west + 3.0, east - 2.0] {
        gfx::line(vec2(x, front), vec2(x, front + legs), 1.8, frame);
    }
    // Rolled towel for a pillow.
    let pillow = hinge_back.lerp(hinge_front, 0.5) + rise * 0.85;
    gfx::ellipse(pillow, 3.0, 5.5, CREAM);
    gfx::ellipse_outline(pillow, 3.0, 5.5, 1.0, gfx::with_alpha(INK, 0.5));
}

fn draw_cooler(s: Vec2, color: Color) {
    gfx::ground_shadow(s + vec2(5.0, 1.5), 12.0, 4.0);
    draw_rectangle(s.x - 9.0, s.y - 11.0, 18.0, 11.0, color);
    draw_rectangle(s.x - 9.0, s.y - 11.0, 4.0, 11.0, gfx::shade(color, 1.15));
    draw_rectangle(s.x - 9.5, s.y - 17.0, 19.0, 6.0, WHITE);
    draw_rectangle_lines(s.x - 9.0, s.y - 11.0, 18.0, 11.0, 1.2, INK);
    draw_rectangle_lines(s.x - 9.5, s.y - 17.0, 19.0, 6.0, 1.2, INK);
    gfx::line(
        vec2(s.x - 4.0, s.y - 14.5),
        vec2(s.x + 4.0, s.y - 14.5),
        2.0,
        gfx::shade(color, 0.7),
    );
}

fn draw_tuft(s: Vec2, size: f32, phase: f32, t: f32) {
    let dark = Color::new(0.40, 0.58, 0.25, 1.0);
    let light = Color::new(0.56, 0.72, 0.32, 1.0);
    for i in 0..5 {
        let fi = i as f32 - 2.0;
        let sway = 0.07 * (t * 1.6 + phase + fi).sin();
        let a = -PI / 2.0 + fi * 0.33 + sway;
        let len = size * (0.75 + 0.25 * ((i * 7 % 3) as f32 / 2.0));
        let base = s + vec2(fi * 1.6, 0.0);
        let pts = [
            base,
            base + dir(a) * len * 0.55 + vec2(fi * 0.6, 0.0),
            base + dir(a + fi * 0.12) * len,
        ];
        gfx::ribbon(
            &pts,
            &[1.8, 1.1, 0.2],
            if i % 2 == 0 { dark } else { light },
        );
    }
}

fn draw_palm(s: Vec2, h: f32, lean: f32, phase: f32, t: f32) {
    let sway = (t * 1.1 + phase).sin();
    let trunk = Color::new(0.64, 0.46, 0.27, 1.0);

    // The crown's shadow falls down-right of the trunk, like late sun.
    let shadow_c = s + vec2(h * 0.42, 10.0);
    gfx::ground_shadow(shadow_c, 36.0, 13.0);
    gfx::line(s, shadow_c - vec2(18.0, 4.0), 5.0, gfx::SHADOW);
    gfx::ground_shadow(s + vec2(2.0, 1.0), 9.0, 3.5);

    let n = 8;
    let pts: Vec<Vec2> = (0..=n)
        .map(|i| {
            let u = i as f32 / n as f32;
            s + vec2(lean * h * u.powf(1.7) + sway * 1.5 * u * u, -h * u)
        })
        .collect();
    let widths: Vec<f32> = (0..=n).map(|i| 6.0 - 2.2 * i as f32 / n as f32).collect();
    let outline: Vec<f32> = widths.iter().map(|w| w + 1.3).collect();
    gfx::ribbon(&pts, &outline, INK);
    gfx::ribbon(&pts, &widths, trunk);
    // Light side of the trunk, then the ring scars that make it a palm.
    let lit: Vec<Vec2> = pts
        .iter()
        .zip(&widths)
        .map(|(p, w)| *p - vec2(w * 0.45, 0.0))
        .collect();
    let lit_w: Vec<f32> = widths.iter().map(|w| w * 0.35).collect();
    gfx::ribbon(&lit, &lit_w, gfx::shade(trunk, 1.2));
    for i in 1..n {
        let (p, w) = (pts[i], widths[i]);
        gfx::line(
            p + vec2(-w, 1.0),
            p + vec2(w, -1.0),
            1.2,
            gfx::shade(trunk, 0.72),
        );
    }

    let top = pts[n];
    let fronds = 7;
    let angles: Vec<f32> = (0..fronds)
        .map(|j| phase + j as f32 / fronds as f32 * TAU + 0.12 * (t * 1.3 + phase + j as f32).sin())
        .collect();
    // Back fronds (pointing up-screen) go behind the coconuts, front
    // fronds over them.
    for &a in angles.iter().filter(|a| a.sin() < 0.0) {
        draw_frond(top, a, h, 0.72);
    }
    for (dx, dy) in [(-4.0, 4.0), (3.5, 5.0), (0.0, 1.5)] {
        draw_circle(
            top.x + dx,
            top.y + dy,
            4.2,
            Color::new(0.42, 0.28, 0.13, 1.0),
        );
        draw_circle(
            top.x + dx - 1.2,
            top.y + dy - 1.2,
            1.3,
            Color::new(0.62, 0.45, 0.25, 1.0),
        );
    }
    for &a in angles.iter().filter(|a| a.sin() >= 0.0) {
        draw_frond(top, a, h, 1.0);
    }
}

fn draw_frond(top: Vec2, a: f32, h: f32, light: f32) {
    let len = 38.0 + h * 0.12;
    let d = vec2(a.cos(), a.sin() * 0.55);
    let n = 7;
    let pts: Vec<Vec2> = (0..=n)
        .map(|i| {
            let u = i as f32 / n as f32;
            top + d * len * u + vec2(0.0, 0.55 * len * u * u)
        })
        .collect();
    let widths: Vec<f32> = (0..=n)
        .map(|i| {
            let u = i as f32 / n as f32;
            7.0 * (1.0 - u).powf(0.7) * (u * 4.0).min(1.0) + 0.3
        })
        .collect();
    let leaf = gfx::shade(Color::new(0.22, 0.62, 0.28, 1.0), light);
    let outline: Vec<f32> = widths.iter().map(|w| w + 1.1).collect();
    gfx::ribbon(&pts, &outline, gfx::with_alpha(INK, 0.7));
    gfx::ribbon(&pts, &widths, leaf);
    // Serrated leaflet cuts: short dark strokes slanting back from the rib.
    for i in 1..n {
        let (p, w) = (pts[i], widths[i]);
        let along = (pts[i + 1] - pts[i - 1]).normalize_or_zero();
        let across = along.perp();
        for side in [-1.0, 1.0] {
            gfx::line(
                p,
                p + across * (side * w) - along * 2.5,
                0.9,
                gfx::shade(leaf, 0.7),
            );
        }
    }
    gfx::polyline(
        &pts,
        1.2,
        gfx::shade(Color::new(0.55, 0.80, 0.36, 1.0), light),
    );
}

fn draw_umbrella(s: Vec2, canopy: Color, stripe: Color, tilt: f32) {
    let pole_top = s + vec2(tilt * 40.0, -42.0);
    gfx::ground_shadow(s + vec2(16.0, 6.0), 36.0, 12.0);
    gfx::line(s, pole_top, 3.6, INK);
    gfx::line(s, pole_top, 2.0, Color::new(0.96, 0.95, 0.92, 1.0));

    let c = pole_top + vec2(0.0, 2.0);
    let apex = pole_top + vec2(0.0, -9.0);
    let (rx, ry) = (34.0, 14.0);
    let wedges = 10;
    let rim = |a: f32| c + vec2(a.cos() * rx, a.sin() * ry);
    gfx::ellipse(c + vec2(0.0, 1.5), rx + 1.5, ry + 1.5, INK);
    for i in 0..wedges {
        let a0 = i as f32 / wedges as f32 * TAU;
        let a1 = (i + 1) as f32 / wedges as f32 * TAU;
        let am = (a0 + a1) * 0.5;
        // Panels facing the upper-left light are brighter.
        let lit = 1.0 + 0.10 * (-am.cos() - am.sin());
        let col = if i % 2 == 0 { canopy } else { stripe };
        draw_triangle(apex, rim(a0), rim(a1), gfx::shade(col, lit));
    }
    // Scalloped hem along the front edge.
    for i in 0..wedges / 2 {
        let a = (i as f32 + 0.5) / wedges as f32 * TAU;
        let col = if i % 2 == 0 { canopy } else { stripe };
        gfx::ellipse(rim(a) + vec2(0.0, 0.5), 3.4, 2.2, gfx::shade(col, 0.85));
    }
    for i in 0..wedges {
        let a = i as f32 / wedges as f32 * TAU;
        gfx::line(apex, rim(a), 0.8, gfx::with_alpha(INK, 0.25));
    }
    draw_circle(apex.x, apex.y, 2.6, CREAM);
    draw_circle_lines(apex.x, apex.y, 2.6, 1.0, INK);
}

fn draw_sandcastle(s: Vec2, t: f32) {
    gfx::ground_shadow(s + vec2(7.0, 2.0), 30.0, 9.0);
    let outline = gfx::with_alpha(INK, 0.55);

    // Central keep, behind the wall.
    let keep = s + vec2(0.0, -10.0);
    draw_rectangle(keep.x - 8.0, keep.y - 30.0, 16.0, 30.0, SAND_FRONT);
    gfx::ellipse(vec2(keep.x, keep.y - 30.0), 8.0, 3.2, SAND_TOP);
    draw_rectangle_lines(keep.x - 8.0, keep.y - 30.0, 16.0, 30.0, 1.0, outline);
    let pole = vec2(keep.x, keep.y - 32.0);
    gfx::line(pole, pole + vec2(0.0, -14.0), 1.4, INK);
    let wave = 2.0 * (t * 5.0).sin();
    draw_triangle(
        pole + vec2(0.0, -14.0),
        pole + vec2(0.0, -8.0),
        pole + vec2(10.0, -11.0 + wave),
        CORAL,
    );

    // Curtain wall: front face, top face, crenellations, and a door.
    draw_rectangle(s.x - 20.0, s.y - 20.0, 40.0, 6.0, SAND_TOP);
    for i in 0..5 {
        draw_rectangle(s.x - 18.0 + i as f32 * 8.0, s.y - 24.0, 5.0, 4.0, SAND_TOP);
    }
    draw_rectangle(s.x - 20.0, s.y - 14.0, 40.0, 14.0, SAND_FRONT);
    draw_rectangle_lines(s.x - 20.0, s.y - 20.0, 40.0, 20.0, 1.0, outline);
    gfx::ellipse(
        vec2(s.x, s.y - 7.0),
        4.0,
        4.0,
        Color::new(0.50, 0.36, 0.20, 1.0),
    );
    draw_rectangle(
        s.x - 4.0,
        s.y - 7.0,
        8.0,
        7.0,
        Color::new(0.50, 0.36, 0.20, 1.0),
    );

    // Corner towers.
    for dx in [-20.0, 20.0] {
        let x = s.x + dx;
        draw_rectangle(
            x - 7.0,
            s.y - 26.0,
            14.0,
            26.0,
            gfx::shade(SAND_FRONT, 0.96),
        );
        gfx::ellipse(vec2(x, s.y - 26.0), 7.0, 2.8, SAND_TOP);
        for cx in [-5.0, 0.0, 5.0] {
            draw_rectangle(x + cx - 1.5, s.y - 30.0, 3.0, 3.0, SAND_TOP);
        }
        draw_rectangle_lines(x - 7.0, s.y - 26.0, 14.0, 26.0, 1.0, outline);
        draw_rectangle(
            x - 1.5,
            s.y - 18.0,
            3.0,
            4.0,
            Color::new(0.50, 0.36, 0.20, 1.0),
        );
    }
    // A shell pressed into the wall.
    draw_circle(s.x + 10.0, s.y - 9.0, 2.0, PINK);
}

fn draw_bucket(s: Vec2, color: Color) {
    gfx::ground_shadow(s + vec2(5.0, 1.0), 11.0, 3.5);
    // Spade lying beside it.
    let handle = Color::new(1.0, 0.85, 0.30, 1.0);
    gfx::line(s + vec2(9.0, 1.0), s + vec2(20.0, -3.0), 2.2, handle);
    gfx::quad(
        s + vec2(19.0, -6.0),
        s + vec2(26.0, -7.0),
        s + vec2(27.0, -1.0),
        s + vec2(20.0, 0.0),
        gfx::shade(color, 0.8),
    );

    let (tl, tr, br, bl) = (
        s + vec2(-7.0, -15.0),
        s + vec2(7.0, -15.0),
        s + vec2(5.0, 0.0),
        s + vec2(-5.0, 0.0),
    );
    gfx::quad(tl, tr, br, bl, color);
    gfx::quad(
        tl,
        tl + vec2(4.0, 0.0),
        bl + vec2(3.0, 0.0),
        bl,
        gfx::shade(color, 1.18),
    );
    gfx::polyline(&[tl, bl, br, tr], 1.2, INK);
    gfx::ellipse(s + vec2(0.0, -15.0), 7.0, 2.4, gfx::shade(color, 0.6));
    gfx::ellipse_outline(s + vec2(0.0, -15.0), 7.0, 2.4, 1.2, INK);
    draw_arc(s.x, s.y - 15.0, 12, 6.5, 190.0, 1.1, 160.0, INK);
}

fn draw_surfboard(s: Vec2, color: Color, stripe: Color, tilt: f32) {
    let up = vec2(tilt.sin(), -tilt.cos());
    let center = s + up * 26.0;
    let deg = tilt.to_degrees();
    gfx::ellipse_rot(s + vec2(20.0, 4.0), 24.0, 5.0, 10.0, gfx::SHADOW);
    gfx::ellipse_rot(center, 7.5, 28.0, deg, color);
    gfx::ellipse_rot(
        center - vec2(1.5, 0.0),
        4.0,
        24.0,
        deg,
        gfx::shade(color, 1.15),
    );
    gfx::ellipse_rot(center, 1.6, 25.0, deg, stripe);
    draw_ellipse_lines(center.x, center.y, 7.5, 28.0, deg, 1.5, INK);
    // Sand heaped where it's stuck in.
    gfx::ellipse(s + vec2(0.0, 1.0), 10.0, 3.5, SAND_FRONT);
    gfx::ellipse(s + vec2(-1.0, 0.0), 8.0, 2.5, SAND_TOP);
}

fn draw_beach_ball(s: Vec2, phase: f32, t: f32) {
    let c = s + vec2(0.0, -9.0);
    let r = 9.0;
    let spin = phase + 0.25 * (t * 1.3 + phase).sin();
    gfx::ground_shadow(s + vec2(3.0, 1.0), 9.0, 3.0);
    let colors = [CORAL, WHITE, SUNNY, WHITE, SKY, WHITE];
    for (i, col) in colors.iter().enumerate() {
        for sub in 0..4 {
            let a0 = spin + (i as f32 + sub as f32 / 4.0) / 6.0 * TAU;
            let a1 = spin + (i as f32 + (sub + 1) as f32 / 4.0) / 6.0 * TAU;
            draw_triangle(c, c + dir(a0) * r, c + dir(a1) * r, *col);
        }
    }
    draw_circle(c.x, c.y, 2.2, WHITE);
    gfx::ellipse(c + vec2(-3.0, -3.5), 3.0, 1.8, gfx::with_alpha(WHITE, 0.6));
    draw_circle_lines(c.x, c.y, r, 1.4, INK);
}

fn draw_crab(s: Vec2, phase: f32, t: f32) {
    let walk = (t * 0.7 + phase).sin();
    let speed = (t * 0.7 + phase).cos().abs();
    let c = s + vec2(14.0 * walk, -4.0);
    let shell = Color::new(0.93, 0.36, 0.22, 1.0);
    gfx::ground_shadow(s + vec2(14.0 * walk + 1.0, 1.0), 10.0, 3.0);

    for side in [-1.0f32, 1.0] {
        for j in 0..3 {
            let wiggle = speed * 2.0 * (t * 14.0 + j as f32 * 2.0 + side).sin();
            let from = c + vec2(side * 5.0, 1.0);
            let knee = c + vec2(
                side * (9.0 + j as f32),
                -1.0 + j as f32 * 2.0 + wiggle * 0.5,
            );
            let foot = c + vec2(
                side * (12.0 + j as f32 * 1.5),
                3.5 + j as f32 * 2.0 + wiggle,
            );
            gfx::polyline(&[from, knee, foot], 1.4, gfx::shade(shell, 0.75));
        }
        let arm = c + vec2(side * 9.0, -6.0);
        gfx::line(
            c + vec2(side * 5.0, -2.0),
            arm,
            2.0,
            gfx::shade(shell, 0.85),
        );
        draw_circle(arm.x, arm.y, 3.4, shell);
        draw_triangle(
            arm + vec2(side * 0.5, -3.6),
            arm + vec2(side * 3.6, -1.0),
            arm + vec2(side * 1.0, -0.5),
            Color::new(0.97, 0.88, 0.66, 1.0),
        );
    }
    gfx::ellipse(c, 8.0, 5.5, shell);
    gfx::ellipse(c + vec2(-1.5, -1.5), 5.0, 2.6, gfx::shade(shell, 1.18));
    gfx::ellipse_outline(c, 8.0, 5.5, 1.1, INK);
    for side in [-1.0, 1.0] {
        let eye = c + vec2(side * 3.0, -9.0);
        gfx::line(c + vec2(side * 2.5, -4.0), eye, 1.2, gfx::shade(shell, 0.8));
        draw_circle(eye.x, eye.y, 2.0, WHITE);
        draw_circle(eye.x, eye.y, 1.0, INK);
    }
}

fn draw_rock(s: Vec2, size: f32, wet: bool, t: f32) {
    let base = if wet {
        Color::new(0.46, 0.47, 0.50, 1.0)
    } else {
        Color::new(0.62, 0.58, 0.54, 1.0)
    };
    if wet {
        let p = 1.0 + 0.12 * (t * 1.7 + s.x * 0.05).sin();
        gfx::ellipse(
            s,
            size * 1.5 * p,
            size * 0.55 * p,
            gfx::with_alpha(WHITE, 0.25),
        );
    } else {
        gfx::ground_shadow(s + vec2(4.0, 2.0), size * 1.15, size * 0.4);
    }
    let body = s + vec2(0.0, -size * 0.35);
    gfx::ellipse(body, size, size * 0.62, gfx::shade(base, 0.82));
    gfx::ellipse(
        s + vec2(-size * 0.55, -size * 0.15),
        size * 0.6,
        size * 0.42,
        gfx::shade(base, 0.74),
    );
    gfx::ellipse(
        s + vec2(-size * 0.15, -size * 0.55),
        size * 0.7,
        size * 0.38,
        base,
    );
    gfx::ellipse(
        s + vec2(-size * 0.3, -size * 0.72),
        size * 0.3,
        size * 0.14,
        gfx::shade(base, 1.3),
    );
    gfx::ellipse_outline(body, size, size * 0.62, 1.2, gfx::with_alpha(INK, 0.55));
    if wet {
        gfx::ellipse_outline(
            s + vec2(0.0, 1.0),
            size * 1.1,
            size * 0.32,
            2.0,
            gfx::with_alpha(WHITE, 0.8),
        );
    }
}

fn ripple_ring(s: Vec2, rx: f32, ry: f32, phase: f32, t: f32) {
    let grow = 1.0 + 0.12 * (t * 2.0 + phase).sin();
    gfx::ellipse_outline(s, rx * grow, ry * grow, 1.5, gfx::with_alpha(WHITE, 0.45));
}

fn draw_buoy(s: Vec2, phase: f32, t: f32) {
    let bob = 2.0 * (t * 2.0 + phase).sin();
    let c = s + vec2(0.0, -6.0 + bob);
    let red = Color::new(0.92, 0.25, 0.22, 1.0);
    ripple_ring(s, 12.0, 4.5, phase, t);
    gfx::ellipse(c, 6.5, 7.5, red);
    draw_rectangle(c.x - 6.2, c.y - 2.0, 12.4, 4.0, WHITE);
    draw_triangle(
        c + vec2(-3.0, -6.5),
        c + vec2(3.0, -6.5),
        c + vec2(0.0, -13.0),
        red,
    );
    gfx::ellipse_outline(c, 6.5, 7.5, 1.2, INK);
    let blink = (t * 2.5 + phase).sin() > 0.6;
    if blink {
        draw_circle(c.x, c.y - 14.0, 4.0, gfx::with_alpha(SUNNY, 0.35));
    }
    draw_circle(c.x, c.y - 14.0, 1.8, SUNNY);
    gfx::ellipse(
        s + vec2(0.0, 1.5),
        7.0,
        2.2,
        gfx::with_alpha(WATER_MID, 0.85),
    );
}

fn draw_sailboat(s: Vec2, phase: f32, sail: Color, t: f32) {
    let c = s + vec2(
        18.0 * (t * 0.12 + phase).sin(),
        1.5 * (t * 1.6 + phase).sin(),
    );
    let lean = 2.0 * (t * 1.1 + phase).sin();
    ripple_ring(c + vec2(0.0, 2.0), 28.0, 6.5, phase, t);
    gfx::quad(
        c + vec2(-22.0, -6.0),
        c + vec2(22.0, -6.0),
        c + vec2(15.0, 4.0),
        c + vec2(-15.0, 4.0),
        Color::new(0.97, 0.95, 0.90, 1.0),
    );
    gfx::quad(
        c + vec2(-18.0, 0.0),
        c + vec2(18.0, 0.0),
        c + vec2(15.0, 4.0),
        c + vec2(-15.0, 4.0),
        sail,
    );
    gfx::polyline(
        &[
            c + vec2(-22.0, -6.0),
            c + vec2(-15.0, 4.0),
            c + vec2(15.0, 4.0),
            c + vec2(22.0, -6.0),
            c + vec2(-22.0, -6.0),
        ],
        1.3,
        INK,
    );
    let mast_top = c + vec2(lean, -50.0);
    gfx::line(c + vec2(0.0, -6.0), mast_top, 2.0, gfx::shade(WOOD, 0.7));
    let main = [
        mast_top + vec2(2.0, 3.0),
        c + vec2(2.0, -9.0),
        c + vec2(25.0, -9.0),
    ];
    draw_triangle(main[0], main[1], main[2], CREAM);
    let (head, tack, clew) = (main[0], main[1], main[2]);
    gfx::quad(
        tack.lerp(head, 0.22),
        clew.lerp(head, 0.22),
        clew.lerp(head, 0.38),
        tack.lerp(head, 0.38),
        sail,
    );
    gfx::polyline(&[main[0], main[1], main[2], main[0]], 1.2, INK);
    let jib = [
        mast_top + vec2(-2.0, 6.0),
        c + vec2(-2.0, -9.0),
        c + vec2(-18.0, -9.0),
    ];
    draw_triangle(jib[0], jib[1], jib[2], sail);
    gfx::polyline(&[jib[0], jib[1], jib[2], jib[0]], 1.2, INK);
    draw_triangle(
        mast_top,
        mast_top + vec2(0.0, 5.0),
        mast_top + vec2(8.0, 2.5 + lean),
        CORAL,
    );
}

fn draw_swim_ring(s: Vec2, phase: f32, color: Color, t: f32) {
    let c = s + vec2(0.0, -2.0 + 1.5 * (t * 1.8 + phase).sin());
    ripple_ring(s, 16.0, 5.5, phase, t);
    gfx::ellipse(c, 12.0, 7.0, color);
    for i in 0..4 {
        let a = phase + PI / 4.0 + i as f32 * PI / 2.0;
        let p = c + vec2(a.cos() * 9.0, a.sin() * 5.2);
        gfx::ellipse_rot(p, 3.4, 2.4, a.to_degrees() + 90.0, WHITE);
    }
    gfx::ellipse(c, 5.0, 2.6, Color::new(0.28, 0.78, 0.84, 1.0));
    gfx::ellipse(c + vec2(-5.0, -3.0), 3.0, 1.2, gfx::with_alpha(WHITE, 0.6));
    gfx::ellipse_outline(c, 12.0, 7.0, 1.3, INK);
}

fn draw_hut(s: Vec2, flip: bool) {
    let f = if flip { -1.0 } else { 1.0 };
    let bamboo = Color::new(0.80, 0.62, 0.36, 1.0);
    let dark = Color::new(0.30, 0.20, 0.12, 1.0);
    gfx::ground_shadow(s + vec2(14.0, 3.0), 48.0, 12.0);

    // Raised floor and walls.
    draw_rectangle(s.x - 34.0, s.y - 8.0, 68.0, 8.0, gfx::shade(WOOD, 0.8));
    draw_rectangle(s.x - 30.0, s.y - 38.0, 60.0, 30.0, bamboo);
    for i in 0..10 {
        let x = s.x - 30.0 + i as f32 * 6.0;
        gfx::line(
            vec2(x, s.y - 38.0),
            vec2(x, s.y - 8.0),
            1.0,
            gfx::shade(bamboo, 0.78),
        );
    }
    draw_rectangle(s.x - 30.0, s.y - 38.0, 6.0, 30.0, gfx::shade(bamboo, 1.12));
    draw_rectangle_lines(s.x - 30.0, s.y - 38.0, 60.0, 30.0, 1.4, INK);
    draw_rectangle_lines(s.x - 34.0, s.y - 8.0, 68.0, 8.0, 1.4, INK);
    let door_x = s.x - 7.0 - f * 11.0;
    draw_rectangle(door_x, s.y - 30.0, 14.0, 22.0, dark);
    let win_x = s.x - 6.0 + f * 14.0;
    draw_rectangle(
        win_x,
        s.y - 31.0,
        12.0,
        10.0,
        Color::new(0.15, 0.32, 0.36, 1.0),
    );
    gfx::line(
        vec2(win_x + 6.0, s.y - 31.0),
        vec2(win_x + 6.0, s.y - 21.0),
        1.2,
        bamboo,
    );
    gfx::line(
        vec2(win_x, s.y - 26.0),
        vec2(win_x + 12.0, s.y - 26.0),
        1.2,
        bamboo,
    );
    draw_rectangle_lines(win_x, s.y - 31.0, 12.0, 10.0, 1.2, INK);

    // Thatched roof with a ragged fringe.
    let (bl, br, tr, tl) = (
        s + vec2(-42.0, -36.0),
        s + vec2(42.0, -36.0),
        s + vec2(24.0, -66.0),
        s + vec2(-24.0, -66.0),
    );
    gfx::quad(bl, br, tr, tl, STRAW);
    gfx::quad(
        tl,
        tr,
        tr + vec2(0.0, 6.0),
        tl + vec2(0.0, 6.0),
        gfx::shade(STRAW, 1.12),
    );
    for i in 0..12 {
        let u = (i as f32 + 0.5) / 12.0;
        gfx::line(tl.lerp(tr, u), bl.lerp(br, u), 1.0, gfx::shade(STRAW, 0.8));
    }
    for i in 0..14 {
        let x = bl.x + i as f32 * 6.0;
        draw_triangle(
            vec2(x, bl.y),
            vec2(x + 6.0, bl.y),
            vec2(x + 3.0, bl.y + 6.0 + (i % 2) as f32 * 2.0),
            gfx::shade(STRAW, 0.88),
        );
    }
    gfx::polyline(&[bl, tl, tr, br], 1.4, INK);
}

fn draw_torch(s: Vec2, phase: f32, t: f32) {
    let h = 36.0;
    let top = s + vec2(0.0, -h);
    let flick = 0.5 + 0.5 * (t * 11.0 + phase).sin() * (t * 7.3 + phase * 1.7).sin();
    let sway = 1.5 * (t * 5.0 + phase).sin();

    draw_circle(
        top.x,
        top.y - 8.0,
        22.0 + flick * 5.0,
        Color::new(1.0, 0.65, 0.25, 0.10),
    );
    gfx::ground_shadow(s + vec2(4.0, 1.0), 6.0, 2.5);
    let pole = Color::new(0.72, 0.55, 0.30, 1.0);
    draw_rectangle(s.x - 2.2, top.y, 4.4, h, pole);
    for k in 1..4 {
        let y = top.y + k as f32 * h / 4.0;
        gfx::line(
            vec2(s.x - 2.4, y),
            vec2(s.x + 2.4, y),
            1.4,
            gfx::shade(pole, 0.65),
        );
    }
    draw_rectangle_lines(s.x - 2.2, top.y, 4.4, h, 1.0, INK);
    gfx::quad(
        top + vec2(-5.5, -3.0),
        top + vec2(5.5, -3.0),
        top + vec2(3.5, 5.0),
        top + vec2(-3.5, 5.0),
        Color::new(0.45, 0.30, 0.16, 1.0),
    );
    gfx::line(
        top + vec2(-4.5, 1.0),
        top + vec2(4.5, 1.0),
        1.0,
        gfx::shade(pole, 0.9),
    );

    let flame = |scale: f32, color: Color| {
        let base = top + vec2(0.0, -4.0);
        draw_circle(base.x, base.y, 5.0 * scale, color);
        draw_triangle(
            base + vec2(-5.0 * scale, 0.0),
            base + vec2(5.0 * scale, 0.0),
            base + vec2(sway * scale, -(12.0 + flick * 5.0) * scale),
            color,
        );
    };
    flame(1.0, Color::new(1.0, 0.42, 0.12, 1.0));
    flame(0.68, Color::new(1.0, 0.75, 0.20, 1.0));
    flame(0.36, Color::new(1.0, 0.96, 0.75, 1.0));
}

fn draw_hibiscus(s: Vec2, flower: Color) {
    gfx::ground_shadow(s + vec2(6.0, 2.0), 22.0, 6.0);
    let dark = Color::new(0.16, 0.48, 0.24, 1.0);
    let mid = Color::new(0.24, 0.60, 0.28, 1.0);
    let light = Color::new(0.36, 0.70, 0.32, 1.0);
    for (dx, dy, r, c) in [
        (-11.0, -9.0, 11.0, dark),
        (10.0, -10.0, 11.0, dark),
        (0.0, -18.0, 12.0, mid),
        (-6.0, -7.0, 9.0, mid),
        (6.0, -6.0, 8.0, light),
        (-3.0, -17.0, 6.0, light),
    ] {
        gfx::ellipse(s + vec2(dx, dy), r, r * 0.8, c);
    }
    for (dx, dy) in [(-9.0, -14.0), (8.0, -18.0), (2.0, -6.0)] {
        let c = s + vec2(dx, dy);
        for i in 0..5 {
            let a = i as f32 / 5.0 * TAU - PI / 2.0;
            gfx::ellipse_rot(c + dir(a) * 3.2, 3.4, 2.4, a.to_degrees(), flower);
        }
        draw_circle(c.x, c.y, 1.6, SUNNY);
    }
}

fn draw_lifeguard(s: Vec2, t: f32) {
    let white = Color::new(0.97, 0.96, 0.93, 1.0);
    let red = Color::new(0.90, 0.26, 0.22, 1.0);
    gfx::ground_shadow(s + vec2(18.0, 5.0), 32.0, 10.0);

    // Back legs, then front legs with cross-bracing.
    for dx in [-12.0, 12.0] {
        gfx::line(
            s + vec2(dx, -12.0),
            s + vec2(dx * 0.8, -38.0),
            3.0,
            gfx::shade(white, 0.78),
        );
    }
    for dx in [-17.0, 17.0] {
        gfx::line(s + vec2(dx, 0.0), s + vec2(dx * 0.85, -38.0), 3.4, white);
    }
    gfx::line(
        s + vec2(-16.0, -4.0),
        s + vec2(14.5, -34.0),
        1.6,
        gfx::shade(white, 0.85),
    );
    gfx::line(
        s + vec2(16.0, -4.0),
        s + vec2(-14.5, -34.0),
        1.6,
        gfx::shade(white, 0.85),
    );
    // Ladder.
    gfx::line(s + vec2(-5.0, 6.0), s + vec2(-4.0, -38.0), 1.6, WOOD);
    gfx::line(s + vec2(5.0, 6.0), s + vec2(4.0, -38.0), 1.6, WOOD);
    for i in 0..6 {
        let y = 2.0 - i as f32 * 7.0;
        gfx::line(s + vec2(-4.5, y), s + vec2(4.5, y), 1.4, WOOD);
    }

    draw_rectangle(s.x - 21.0, s.y - 41.0, 42.0, 5.0, WOOD);
    draw_rectangle_lines(s.x - 21.0, s.y - 41.0, 42.0, 5.0, 1.2, INK);
    draw_rectangle(s.x - 16.0, s.y - 64.0, 32.0, 23.0, white);
    draw_rectangle(s.x - 16.0, s.y - 48.0, 32.0, 5.0, red);
    draw_rectangle(
        s.x - 11.0,
        s.y - 60.0,
        22.0,
        9.0,
        Color::new(0.20, 0.30, 0.38, 1.0),
    );
    draw_rectangle_lines(s.x - 16.0, s.y - 64.0, 32.0, 23.0, 1.3, INK);
    let roof = [
        s + vec2(-21.0, -63.0),
        s + vec2(21.0, -63.0),
        s + vec2(16.0, -72.0),
        s + vec2(-16.0, -72.0),
    ];
    gfx::quad(roof[0], roof[1], roof[2], roof[3], red);
    gfx::polyline(&[roof[0], roof[1], roof[2], roof[3], roof[0]], 1.3, INK);

    // Life ring hung on the cabin.
    draw_circle(
        s.x + 21.0,
        s.y - 52.0,
        5.0,
        Color::new(1.0, 0.55, 0.18, 1.0),
    );
    draw_circle(s.x + 21.0, s.y - 52.0, 2.2, white);

    let pole = s + vec2(-19.0, -72.0);
    gfx::line(pole, pole + vec2(0.0, -22.0), 1.6, INK);
    let wave = 3.0 * (t * 6.0).sin();
    let ft = pole + vec2(0.0, -22.0);
    draw_triangle(
        ft,
        ft + vec2(0.0, 10.0),
        ft + vec2(-15.0, 5.0 + wave),
        SUNNY,
    );
    draw_triangle(
        ft + vec2(0.0, 5.0),
        ft + vec2(0.0, 10.0),
        ft + vec2(-15.0, 5.0 + wave),
        red,
    );
}

fn wood_board(x: f32, y: f32, w: f32, h: f32) {
    let board = Color::new(0.80, 0.62, 0.40, 1.0);
    draw_rectangle(x, y, w, h, board);
    draw_rectangle(x, y, w, 2.5, gfx::shade(board, 1.15));
    draw_rectangle_lines(x, y, w, h, 1.5, INK);
}

fn centered_text(text: &str, cx: f32, baseline: f32, size: u16, color: Color) {
    let dims = measure_text(text, None, size, 1.0);
    draw_text(
        text,
        (cx - dims.width * 0.5).round(),
        baseline.round(),
        size as f32,
        color,
    );
}

fn draw_marker(s: Vec2, meters: u32) {
    gfx::ground_shadow(s + vec2(4.0, 1.0), 9.0, 2.5);
    draw_rectangle(s.x - 2.5, s.y - 26.0, 5.0, 26.0, WOOD);
    draw_rectangle_lines(s.x - 2.5, s.y - 26.0, 5.0, 26.0, 1.0, INK);
    wood_board(s.x - 19.0, s.y - 40.0, 38.0, 16.0);
    centered_text(
        &format!("{meters}m"),
        s.x,
        s.y - 28.0,
        16,
        Color::new(0.30, 0.18, 0.08, 1.0),
    );
}

fn draw_signpost(s: Vec2) {
    gfx::ground_shadow(s + vec2(8.0, 2.0), 44.0, 6.0);
    for dx in [-32.0, 32.0] {
        draw_rectangle(s.x + dx - 2.5, s.y - 34.0, 5.0, 34.0, WOOD);
        draw_rectangle_lines(s.x + dx - 2.5, s.y - 34.0, 5.0, 34.0, 1.0, INK);
    }
    wood_board(s.x - 50.0, s.y - 60.0, 100.0, 30.0);
    let ink = Color::new(0.30, 0.18, 0.08, 1.0);
    centered_text("JEKO BEACH", s.x - 8.0, s.y - 40.0, 18, ink);
    // Painted arrow pointing up the boardwalk.
    let a = vec2(s.x + 38.0, s.y - 45.0);
    draw_rectangle(a.x - 2.0, a.y - 2.0, 4.0, 10.0, CORAL);
    draw_triangle(
        a + vec2(-6.5, -1.0),
        a + vec2(6.5, -1.0),
        a + vec2(0.0, -10.0),
        CORAL,
    );
}

/// The hub's gateway: two tall posts either side of the boardwalk's south
/// steps and a painted board across the top. The player walks under it.
fn draw_gateway(s: Vec2, t: f32) {
    let half = PATH_HALF_W + 12.0;
    let top = 100.0;
    for side in [-1.0f32, 1.0] {
        let x = s.x + side * half;
        gfx::ground_shadow(vec2(x + 6.0, s.y + 2.0), 12.0, 4.0);
        draw_rectangle(x - 4.5, s.y - top, 9.0, top, WOOD);
        draw_rectangle(x - 4.5, s.y - top, 3.0, top, gfx::shade(WOOD, 1.15));
        draw_rectangle_lines(x - 4.5, s.y - top, 9.0, top, 1.5, INK);
        // A little flag on each post, flapping.
        let flag = vec2(x, s.y - top - 6.0);
        let flap = (t * 4.0 + side).sin() * 3.0;
        draw_line(flag.x, flag.y + 6.0, flag.x, flag.y - 14.0, 2.0, INK);
        draw_triangle(
            flag + vec2(0.0, -14.0),
            flag + vec2(0.0, -4.0),
            flag + vec2(side * 16.0, -9.0 + flap),
            if side < 0.0 { CORAL } else { TEAL },
        );
    }
    // Crossbeam and the sign hung from it.
    draw_rectangle(
        s.x - half - 6.0,
        s.y - top + 4.0,
        half * 2.0 + 12.0,
        7.0,
        WOOD,
    );
    draw_rectangle_lines(
        s.x - half - 6.0,
        s.y - top + 4.0,
        half * 2.0 + 12.0,
        7.0,
        1.5,
        INK,
    );
    wood_board(s.x - 62.0, s.y - top + 14.0, 124.0, 26.0);
    let ink = Color::new(0.30, 0.18, 0.08, 1.0);
    centered_text("THE ROAD", s.x - 8.0, s.y - top + 33.0, 18, ink);
    let a = vec2(s.x + 46.0, s.y - top + 28.0);
    draw_rectangle(a.x - 2.0, a.y - 2.0, 4.0, 9.0, CORAL);
    draw_triangle(
        a + vec2(-6.0, -1.0),
        a + vec2(6.0, -1.0),
        a + vec2(0.0, -9.0),
        CORAL,
    );
}

/// Professor Pebble: a round grey beach pebble with spectacles, a bushy
/// moustache and a tiny mortarboard, bobbing on stubby legs.
fn draw_pebble(s: Vec2, t: f32) {
    let stone = Color::new(0.70, 0.70, 0.68, 1.0);
    let bob = (t * 2.2).sin().abs() * 2.0;
    gfx::ground_shadow(s + vec2(4.0, 2.0), 18.0, 5.0);
    for dx in [-6.0f32, 6.0] {
        gfx::line(s + vec2(dx, -8.0 - bob), s + vec2(dx, -1.0), 3.4, INK);
        gfx::ellipse(s + vec2(dx, -1.0), 3.6, 1.8, INK);
    }
    let c = s + vec2(0.0, -24.0 - bob);
    gfx::ellipse(c, 18.0, 16.0, stone);
    gfx::ellipse(c + vec2(-5.0, -6.0), 7.0, 4.0, gfx::shade(stone, 1.15));
    gfx::ellipse_outline(c, 18.0, 16.0, 2.2, INK);
    // Spectacles.
    for dx in [-6.0f32, 6.0] {
        draw_circle(c.x + dx, c.y - 2.0, 4.6, Color::new(0.85, 0.95, 1.0, 0.8));
        draw_circle_lines(c.x + dx, c.y - 2.0, 4.6, 1.4, INK);
        draw_circle(c.x + dx, c.y - 1.5, 1.6, INK);
    }
    gfx::line(c + vec2(-1.4, -2.5), c + vec2(1.4, -2.5), 1.2, INK);
    // Moustache.
    let tash = Color::new(0.95, 0.95, 0.92, 1.0);
    gfx::ellipse_rot(c + vec2(-4.0, 6.0), 5.0, 2.2, 15.0, tash);
    gfx::ellipse_rot(c + vec2(4.0, 6.0), 5.0, 2.2, -15.0, tash);
    // Mortarboard with a swinging tassel.
    let top = c + vec2(0.0, -16.0);
    gfx::quad(
        top + vec2(-12.0, 0.0),
        top + vec2(0.0, -5.0),
        top + vec2(12.0, 0.0),
        top + vec2(0.0, 5.0),
        Color::new(0.20, 0.22, 0.30, 1.0),
    );
    draw_rectangle(
        top.x - 6.0,
        top.y,
        12.0,
        4.0,
        Color::new(0.20, 0.22, 0.30, 1.0),
    );
    let swing = (t * 3.0).sin() * 3.0;
    gfx::line(top, top + vec2(9.0 + swing, 7.0), 1.2, SUNNY);
}
