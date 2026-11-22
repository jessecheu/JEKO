//! Forest scenery (FR24): what gets scattered along each chunk of the road,
//! and how each prop is drawn.
//!
//! Generation works in zones that are defined relative to the path and the
//! lake's bank at each `y` (open water, the shallows, the muddy bank, the
//! lakeside between the bank and the road, the verge right of the road, and
//! the woods), so props follow the path as it winds and never land in the
//! water by mistake or on the road.

use super::gfx::{self, INK, LAB_GREEN, LANTERN, MOON};
use super::regions;
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
    /// A fallen leaf.
    Leaf {
        rot: f32,
        tint: Color,
    },
    /// A trail of animal paw prints heading off at `angle` radians.
    Pawprints {
        angle: f32,
    },
    GrassTuft {
        size: f32,
        phase: f32,
    },
    /// A cluster of tiny toadstools; `glow` ones shine faintly.
    Toadstools {
        glow: bool,
    },
    LilyPad {
        rot: f32,
        flower: bool,
    },
    /// A slow ring spreading on the still lake.
    WaterRipple {
        phase: f32,
    },
    /// A will-o'-wisp drifting over the water.
    Wisp {
        phase: f32,
    },

    // Tall: depth-sorted against the player by their base `y`.
    Pine {
        height: f32,
        phase: f32,
    },
    DeadTree {
        height: f32,
        lean: f32,
        phase: f32,
    },
    Bush {
        berries: Color,
    },
    /// A giant toadstool.
    Mushroom {
        cap: Color,
        size: f32,
    },
    Gravestone {
        style: u8,
        tilt: f32,
    },
    /// A broken stone column.
    Pillar,
    Cottage {
        flip: bool,
    },
    /// An iron lamppost with a hanging lantern.
    Lamppost {
        phase: f32,
    },
    Pumpkin {
        lit: bool,
        phase: f32,
    },
    /// A bubbling green cauldron: the doctor has been here.
    Cauldron {
        phase: f32,
    },
    Stump {
        size: f32,
    },
    Rock {
        size: f32,
        wet: bool,
    },
    Rowboat {
        phase: f32,
    },
    Reeds {
        phase: f32,
    },
    Crow {
        phase: f32,
    },
    Scarecrow {
        phase: f32,
    },
    Marker {
        meters: u32,
    },
    Signpost,
    /// Professor Pebble, the hub's tutorial NPC (FR9).
    Pebble,
    /// The iron gate at the road's south end: the hub's way onto the road
    /// (FR15).
    Gateway,
    /// The hub camp's fire (FR24).
    Campfire {
        phase: f32,
    },
    /// Professor Pebble's covered wagon in the hub camp.
    Wagon,
    /// The sign at a safe stop (FR28): a lantern board reading "REST".
    RestSign,
    /// The sign at the border of a region (FR27), naming it. `region` indexes
    /// `regions::REGIONS`.
    RegionSign {
        region: u8,
    },
    /// A haystack on the fields outside the village (FR27).
    Haystack {
        flip: bool,
    },
    /// A stone well in the village (FR27).
    Well,
}

pub struct Decoration {
    pub pos: Vec2,
    pub kind: Kind,
}

const BARK: Color = Color::new(0.25, 0.18, 0.14, 1.0);
const STONE: Color = Color::new(0.46, 0.47, 0.53, 1.0);
const MOSS: Color = Color::new(0.24, 0.38, 0.22, 1.0);
const PUMPKIN: Color = Color::new(0.92, 0.48, 0.14, 1.0);
const IRON: Color = Color::new(0.13, 0.13, 0.17, 1.0);
const WOOD: Color = Color::new(0.38, 0.26, 0.18, 1.0);
const CAP_RED: Color = Color::new(0.80, 0.22, 0.26, 1.0);
const CAP_PURPLE: Color = Color::new(0.56, 0.32, 0.72, 1.0);
const CAP_TEAL: Color = Color::new(0.24, 0.70, 0.68, 1.0);
const BERRY: Color = Color::new(0.58, 0.26, 0.70, 1.0);
const CREAM: Color = Color::new(0.93, 0.90, 0.80, 1.0);
const LEAVES: [Color; 3] = [
    Color::new(0.60, 0.30, 0.14, 1.0),
    Color::new(0.66, 0.50, 0.18, 1.0),
    Color::new(0.46, 0.18, 0.20, 1.0),
];
const CAPS: [Color; 3] = [CAP_RED, CAP_PURPLE, CAP_TEAL];

#[derive(Clone, Copy)]
enum Zone {
    /// Open water well off the bank, within the default 960px view: the
    /// rowboat, wet rocks.
    Lake,
    /// Water left of the default view, only seen in a wider window.
    FarLake,
    /// Any on-screen water: ripples and wisps.
    Water,
    FarWater,
    /// Just off the bank: lily pads and reeds.
    Shallows,
    /// The band of mud at the water's edge.
    Bank,
    /// Dry ground between the bank and the road: the graveyard side.
    Lakeside,
    /// The strip between the road and the treeline.
    Verge,
    /// The woods within the default view.
    Woods,
    /// Woods right of the default view, only seen in a wider window.
    FarWoods,
}

/// Valid x range at height `y` for something of footprint radius `r`.
/// Most props go in the zones the default 960px window actually shows;
/// the `Far*` zones get a lighter sprinkling so a maximized window
/// doesn't reveal bare ground and empty water at the edges.
fn zone_range(zone: Zone, y: f32, r: f32) -> (f32, f32) {
    let c = world::path_center(y);
    let bank = world::bank_x(y);
    let trees = world::treeline_x(y);
    let far = 420.0;
    match zone {
        Zone::Lake => (r - 10.0, bank - 80.0 - r),
        Zone::FarLake => (-far, -10.0 - r),
        Zone::Water => (-10.0, bank - 20.0),
        Zone::FarWater => (-far, -10.0),
        Zone::Shallows => (bank - 50.0, bank - 18.0),
        Zone::Bank => (bank + 8.0, bank + 26.0),
        Zone::Lakeside => (bank + 38.0 + r, c - PATH_HALF_W - 22.0 - r),
        Zone::Verge => (c + PATH_HALF_W + 26.0 + r, trees - 6.0 - r),
        Zone::Woods => (trees + 10.0 + r, world::WORLD_W - 6.0),
        Zone::FarWoods => (world::WORLD_W + r, world::WORLD_W + far),
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

    fn speckles(&mut self, zone: Zone, n: usize, dark_bias: f64) {
        self.sprinkle(zone, n, |g| Kind::Speckle {
            size: g.rng.gen_range(1.0..2.6),
            dark: g.chance(dark_bias),
        });
    }

    fn pine(&mut self, zone: Zone, min_h: f32, max_h: f32) {
        self.add(zone, 24.0, |g| Kind::Pine {
            height: g.rng.gen_range(min_h..max_h),
            phase: g.phase(),
        });
    }

    fn dead_tree(&mut self, zone: Zone) {
        self.add(zone, 26.0, |g| Kind::DeadTree {
            height: g.rng.gen_range(62.0..96.0),
            lean: g.rng.gen_range(-0.25..0.25),
            phase: g.phase(),
        });
    }
}

fn leaf(g: &mut Gen) -> Kind {
    Kind::Leaf {
        rot: g.phase(),
        tint: LEAVES[g.rng.gen_range(0..LEAVES.len())],
    }
}

fn mushroom(g: &mut Gen) -> Kind {
    Kind::Mushroom {
        cap: CAPS[g.rng.gen_range(0..CAPS.len())],
        size: g.rng.gen_range(0.8..1.2),
    }
}

fn gravestone(g: &mut Gen) -> Kind {
    Kind::Gravestone {
        style: g.rng.gen_range(0..3),
        tilt: g.rng.gen_range(-0.15..0.15),
    }
}

fn pumpkin(g: &mut Gen) -> Kind {
    Kind::Pumpkin {
        lit: g.chance(0.5),
        phase: g.phase(),
    }
}

/// The spawn hub (FR15), dressed as Professor Pebble's lantern-lit camp at
/// the forest edge (FR24): a fixed layout south of the gate, so every run
/// starts in the same place. `(position, keep-clear radius, kind)`.
fn hub_layout() -> Vec<(Vec2, f32, Kind)> {
    let gate = world::ROAD_SOUTH_END;
    let c = world::path_center(gate);
    let at = |dx: f32, dy: f32| vec2(c + dx, gate + dy);
    let mut v = vec![
        (at(0.0, 2.0), 0.0, Kind::Gateway),
        // Lampposts line the funnel up to the gate.
        (at(-112.0, 70.0), 14.0, Kind::Lamppost { phase: 0.3 }),
        (at(112.0, 70.0), 14.0, Kind::Lamppost { phase: 2.1 }),
        (at(-168.0, 150.0), 14.0, Kind::Lamppost { phase: 4.0 }),
        (at(168.0, 150.0), 14.0, Kind::Lamppost { phase: 5.2 }),
        // The camp: Pebble's wagon and fire on the lake side.
        (at(-176.0, 300.0), 46.0, Kind::Wagon),
        (at(-132.0, 236.0), 22.0, Kind::Campfire { phase: 0.0 }),
        (at(-98.0, 262.0), 14.0, Kind::Stump { size: 9.0 }),
        (at(-170.0, 214.0), 14.0, Kind::Stump { size: 8.0 }),
        // A crooked cottage on the woods side, its pumpkin patch out front.
        (at(212.0, 330.0), 50.0, Kind::Cottage { flip: true }),
        (
            at(160.0, 362.0),
            14.0,
            Kind::Pumpkin {
                lit: true,
                phase: 1.0,
            },
        ),
        (
            at(186.0, 384.0),
            12.0,
            Kind::Pumpkin {
                lit: false,
                phase: 2.0,
            },
        ),
        // Giant toadstools and a scarecrow further back.
        (
            at(-130.0, 500.0),
            28.0,
            Kind::Mushroom {
                cap: CAP_RED,
                size: 1.15,
            },
        ),
        (
            at(150.0, 560.0),
            28.0,
            Kind::Mushroom {
                cap: CAP_TEAL,
                size: 1.0,
            },
        ),
        (at(72.0, 470.0), 14.0, Kind::Toadstools { glow: true }),
        (
            at(250.0, 470.0),
            26.0,
            Kind::Pine {
                height: 110.0,
                phase: 1.4,
            },
        ),
        (
            at(-200.0, 680.0),
            26.0,
            Kind::DeadTree {
                height: 92.0,
                lean: 0.2,
                phase: 3.0,
            },
        ),
        (at(30.0, 690.0), 26.0, Kind::Pillar),
        (at(-60.0, 610.0), 30.0, Kind::Scarecrow { phase: 0.5 }),
        (world::pebble_pos(), 26.0, Kind::Pebble),
        // Welcome sign beside the spawn point.
        (at(112.0, 236.0), 48.0, Kind::Signpost),
    ];
    // Keep a clear walk from the spawn point up to the gate.
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

/// A safe stop's furnishings (FR28), laid by hand around the clearing and
/// varied a little by the stop's number so no two are the same:
/// `(position, keep-clear radius, kind)`. Everything sits on the clearing's
/// flat part, on the woods side of the road.
fn stop_layout(stop: &world::SafeStop) -> Vec<(Vec2, f32, Kind)> {
    let h = |n: u64| gfx::hash01(stop.n as u64 * 53 + n);
    // Each prop is placed from the road's edge at its own y, so a bend in the
    // road doesn't push one onto it.
    let at = |dx: f32, dy: f32| {
        let y = stop.y + dy;
        vec2(world::path_center(y) + PATH_HALF_W + dx, y)
    };
    let house = stop.house();
    let mut v = vec![
        // The cottage, its door facing the road.
        (house, 50.0, Kind::Cottage { flip: h(1) < 0.5 }),
        // The fire in the middle, with stumps to sit on.
        (at(44.0, 32.0), 22.0, Kind::Campfire { phase: h(2) * TAU }),
        (
            at(22.0, 44.0),
            14.0,
            Kind::Stump {
                size: 8.0 + h(3) * 2.0,
            },
        ),
        (
            at(66.0, 52.0),
            14.0,
            Kind::Stump {
                size: 8.0 + h(4) * 2.0,
            },
        ),
        // Lamps at the clearing's two ends, where the fence stops.
        (at(16.0, -62.0), 14.0, Kind::Lamppost { phase: h(5) * TAU }),
        (at(16.0, 64.0), 14.0, Kind::Lamppost { phase: h(6) * TAU }),
        // The sign, by the road so it's seen from the south.
        (at(30.0, 100.0 + h(7) * 8.0), 18.0, Kind::RestSign),
    ];
    // A little extra to tell one stop from the next.
    if h(8) < 0.5 {
        v.push((
            at(112.0, 40.0),
            14.0,
            Kind::Pumpkin {
                lit: h(9) < 0.5,
                phase: h(9) * TAU,
            },
        ));
    } else {
        v.push((at(116.0, 38.0), 14.0, Kind::Toadstools { glow: true }));
    }
    v
}

/// True if `pos` falls inside a safe stop's clearing (with a margin for the
/// props' own size), where random scenery must not grow.
fn in_stop_box(pos: Vec2, stop: &world::SafeStop) -> bool {
    let c = world::path_center(pos.y);
    (pos.y - stop.y).abs() < world::STOP_HALF_LEN + 20.0
        && pos.x > c + PATH_HALF_W - 4.0
        && pos.x < c + PATH_HALF_W + world::STOP_EXTRA_W + 36.0
}

/// The region a point `y` on the road lies in.
fn region_at_y(y: f32) -> &'static regions::Region {
    regions::at((world::ROAD_START_Y - y) / PX_PER_METER)
}

/// Dresses chunk `k` for the region it lies in (FR27), after the forest's own
/// scenery is down: thins what the region has less of, then adds what it has
/// more of. The dark forest's knobs are all neutral, so it is left exactly as
/// it was. Extras come from their own seeded stream, so they never move the
/// forest's props.
fn region_scenery(g: &mut Gen, k: i64, seed: u64) {
    let mid = (g.north + g.south) * 0.5;
    let here = region_at_y(mid).scenery;
    let neutral = regions::REGIONS[0].scenery;

    // Thin the pines and the graveyard's stones by where each one stands.
    g.out.retain(|d| {
        let share = match d.kind {
            Kind::Pine { .. } => region_at_y(d.pos.y).scenery.pines,
            Kind::Gravestone { .. } | Kind::Pillar | Kind::Scarecrow { .. } => {
                region_at_y(d.pos.y).scenery.graves
            }
            _ => return true,
        };
        share >= 1.0
            || gfx::hash01(((d.pos.x.to_bits() as u64) << 32) ^ d.pos.y.to_bits() as u64) < share
    });
    if here == neutral {
        return;
    }

    g.rng = StdRng::seed_from_u64(seed ^ 0x2E61_0A7D ^ k as u64);
    let c = here.cottages as f64;
    for _ in 0..2 {
        if g.chance(c * 0.6) {
            let flip = g.chance(0.5);
            // The verge is narrow, so a cottage goes there only when it fits;
            // the woods always have room.
            if g.add(Zone::Verge, 34.0, |_| Kind::Cottage { flip })
                .is_none()
            {
                g.add(Zone::Woods, 44.0, |_| Kind::Cottage { flip });
            }
        }
    }
    if g.chance(c * 0.4) {
        let flip = g.chance(0.5);
        g.add(Zone::Lakeside, 30.0, |_| Kind::Cottage { flip });
    }
    if g.chance(here.hay as f64) {
        let flip = g.chance(0.5);
        let zone = if g.chance(0.5) {
            Zone::Verge
        } else {
            Zone::Lakeside
        };
        g.add(zone, 26.0, |_| Kind::Haystack { flip });
    }
    if g.chance(here.wells as f64) {
        let zone = if g.chance(0.5) {
            Zone::Verge
        } else {
            Zone::Lakeside
        };
        g.add(zone, 26.0, |_| Kind::Well);
    }
    if g.chance(here.lamps as f64) {
        let y = g.rng.gen_range(g.north + 20.0..g.south - 20.0);
        let side = if g.chance(0.5) { -1.0 } else { 1.0 };
        let lamp = vec2(world::path_center(y) + side * (PATH_HALF_W + 20.0), y);
        if g.taken.iter().all(|&(q, r)| q.distance(lamp) > r + 14.0) {
            g.taken.push((lamp, 14.0));
            let phase = g.phase();
            g.push(lamp, Kind::Lamppost { phase });
        }
    }
    if g.chance(here.pillars as f64) {
        let zone = if g.chance(0.5) {
            Zone::Verge
        } else {
            Zone::Lakeside
        };
        g.add(zone, 26.0, |_| Kind::Pillar);
    }
}

/// All scenery for chunk `k`. `spot` is where this chunk's flask sits (if
/// it has one) so it can be framed by a pair of lampposts. `stop` is the safe
/// stop (FR28) the chunk holds, if any: its clearing is kept free of random
/// scenery and furnished by hand.
pub fn generate_chunk(
    k: i64,
    spot: Option<Vec2>,
    stop: Option<world::SafeStop>,
) -> Vec<Decoration> {
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
    g.speckles(Zone::Lakeside, 26, 0.55);
    g.speckles(Zone::Verge, 16, 0.55);
    g.speckles(Zone::Woods, 18, 0.7);
    g.speckles(Zone::FarWoods, 14, 0.7);
    g.sprinkle(Zone::Lakeside, 5, leaf);
    g.sprinkle(Zone::Verge, 4, leaf);
    g.sprinkle(Zone::Woods, 4, leaf);
    g.sprinkle(Zone::Bank, 1, leaf);
    g.sprinkle(Zone::Water, 6, |g| Kind::WaterRipple { phase: g.phase() });
    g.sprinkle(Zone::FarWater, 5, |g| Kind::WaterRipple {
        phase: g.phase(),
    });
    g.sprinkle(Zone::Water, 2, |g| Kind::Wisp { phase: g.phase() });
    g.sprinkle(Zone::FarWater, 2, |g| Kind::Wisp { phase: g.phase() });
    g.sprinkle(Zone::Shallows, 3, |g| Kind::LilyPad {
        rot: g.phase(),
        flower: g.chance(0.3),
    });
    g.sprinkle(Zone::Lakeside, 1, |g| Kind::Toadstools {
        glow: g.chance(0.4),
    });
    g.sprinkle(Zone::Woods, 2, |g| Kind::Toadstools {
        glow: g.chance(0.5),
    });
    // Something padded down to the water.
    if g.chance(0.35) {
        g.sprinkle(Zone::Lakeside, 1, |g| Kind::Pawprints {
            angle: PI + g.rng.gen_range(-0.5..0.5),
        });
    }
    if g.chance(0.2) {
        g.sprinkle(Zone::Verge, 1, |g| Kind::Pawprints {
            angle: g.rng.gen_range(-0.5..0.5),
        });
    }
    g.sprinkle(Zone::Woods, 14, |g| Kind::GrassTuft {
        size: g.rng.gen_range(9.0..16.0),
        phase: g.phase(),
    });
    g.sprinkle(Zone::FarWoods, 8, |g| Kind::GrassTuft {
        size: g.rng.gen_range(9.0..16.0),
        phase: g.phase(),
    });
    g.sprinkle(Zone::Verge, 2, |g| Kind::GrassTuft {
        size: g.rng.gen_range(8.0..12.0),
        phase: g.phase(),
    });

    // Path-side props first: they're pinned to exact spots, so they claim
    // their footprint before random placement fills in around them.
    if let Some(p) = spot {
        for side in [-1.0, 1.0] {
            let lamp = vec2(world::path_center(p.y) + side * (PATH_HALF_W + 20.0), p.y);
            g.taken.push((lamp, 14.0));
            let phase = g.phase();
            g.push(lamp, Kind::Lamppost { phase });
        }
    }
    if g.chance(0.35) {
        let y = g.rng.gen_range(north..south);
        let lamp = vec2(world::path_center(y) + PATH_HALF_W + 20.0, y);
        if g.taken.iter().all(|&(q, r)| q.distance(lamp) > r + 14.0) {
            g.taken.push((lamp, 14.0));
            let phase = g.phase();
            g.push(lamp, Kind::Lamppost { phase });
        }
    }
    // Milestones every 50 m on the lake side of the road.
    let first_m = ((world::ROAD_START_Y - south) / PX_PER_METER / 50.0)
        .ceil()
        .max(1.0) as u32;
    let mut m = first_m * 50;
    loop {
        let y = world::ROAD_START_Y - m as f32 * PX_PER_METER;
        if y <= north || m as f32 >= world::CASTLE_M {
            break;
        }
        if y < south {
            let marker = vec2(world::path_center(y) - PATH_HALF_W - 20.0, y);
            g.taken.push((marker, 18.0));
            g.push(marker, Kind::Marker { meters: m });
        }
        m += 50;
    }
    // A sign where each region starts (FR27), on the woods side so it doesn't
    // crowd the milestones.
    for border in world::region_borders_in_chunk(k) {
        g.taken.push((border.sign, 34.0));
        g.push(
            border.sign,
            Kind::RegionSign {
                region: border.index as u8,
            },
        );
    }

    // The lake.
    g.maybe(0.35, Zone::Lake, 34.0, |g| Kind::Rowboat {
        phase: g.phase(),
    });
    g.maybe(0.40, Zone::Lake, 16.0, |g| Kind::Rock {
        size: g.rng.gen_range(9.0..15.0),
        wet: true,
    });
    g.maybe(0.25, Zone::FarLake, 40.0, |g| Kind::Rowboat {
        phase: g.phase(),
    });
    g.maybe(0.30, Zone::FarLake, 16.0, |g| Kind::Rock {
        size: g.rng.gen_range(9.0..15.0),
        wet: true,
    });
    let reeds = 1 + g.chance(0.6) as usize;
    for _ in 0..reeds {
        g.add(Zone::Shallows, 12.0, |g| Kind::Reeds { phase: g.phase() });
    }
    g.maybe(0.4, Zone::Bank, 12.0, |g| Kind::Reeds { phase: g.phase() });
    g.maybe(0.35, Zone::Bank, 12.0, |g| Kind::Crow { phase: g.phase() });

    // The lakeside: the old graveyard side of the road.
    let stones = 1 + g.chance(0.5) as usize;
    for _ in 0..stones {
        g.add(Zone::Lakeside, 16.0, gravestone);
    }
    g.maybe(0.45, Zone::Lakeside, 14.0, pumpkin);
    g.maybe(0.40, Zone::Lakeside, 24.0, mushroom);
    g.maybe(0.30, Zone::Lakeside, 16.0, |g| Kind::Stump {
        size: g.rng.gen_range(8.0..12.0),
    });
    g.maybe(0.30, Zone::Lakeside, 20.0, |_| Kind::Bush {
        berries: BERRY,
    });
    g.maybe(0.20, Zone::Lakeside, 26.0, |_| Kind::Pillar);
    g.maybe(0.20, Zone::Lakeside, 14.0, |g| Kind::Rock {
        size: g.rng.gen_range(7.0..12.0),
        wet: false,
    });
    g.maybe(0.10, Zone::Lakeside, 30.0, |g| Kind::Scarecrow {
        phase: g.phase(),
    });
    if g.chance(0.35) {
        g.dead_tree(Zone::Lakeside);
    }

    // The verge between the road and the trees.
    g.maybe(0.35, Zone::Verge, 14.0, pumpkin);
    g.maybe(0.30, Zone::Verge, 16.0, gravestone);
    g.maybe(0.30, Zone::Verge, 22.0, mushroom);
    g.maybe(0.15, Zone::Verge, 18.0, |g| Kind::Cauldron {
        phase: g.phase(),
    });
    g.maybe(0.20, Zone::Verge, 14.0, |g| Kind::Stump {
        size: g.rng.gen_range(7.0..11.0),
    });
    g.maybe(0.15, Zone::Verge, 12.0, |g| Kind::Crow { phase: g.phase() });

    // The woods: a dark wall of pines with the odd cottage.
    g.maybe(0.30, Zone::Woods, 50.0, |g| Kind::Cottage {
        flip: g.chance(0.5),
    });
    let pines = 4 + g.chance(0.6) as usize;
    for _ in 0..pines {
        g.pine(Zone::Woods, 84.0, 132.0);
    }
    if g.chance(0.6) {
        g.dead_tree(Zone::Woods);
    }
    let bushes = 1 + g.chance(0.5) as usize;
    for _ in 0..bushes {
        g.add(Zone::Woods, 20.0, |g| Kind::Bush {
            berries: if g.chance(0.5) { BERRY } else { CAP_RED },
        });
    }
    g.maybe(0.40, Zone::Woods, 16.0, |g| Kind::Rock {
        size: g.rng.gen_range(8.0..14.0),
        wet: false,
    });
    for _ in 0..3 {
        g.pine(Zone::FarWoods, 84.0, 132.0);
    }
    if g.chance(0.4) {
        g.dead_tree(Zone::FarWoods);
    }
    g.maybe(0.4, Zone::FarWoods, 20.0, |_| Kind::Bush { berries: BERRY });
    g.maybe(0.2, Zone::FarWoods, 50.0, |g| Kind::Cottage {
        flip: g.chance(0.5),
    });

    // The open ground below the gate.
    if south > world::ROAD_SOUTH_END {
        let c = world::path_center(world::ROAD_SOUTH_END);
        let plaza = world::ROAD_SOUTH_END;
        g.push(
            vec2(c - 40.0, plaza + 70.0),
            Kind::Toadstools { glow: true },
        );
        g.push(
            vec2(c + 52.0, plaza + 46.0),
            Kind::Leaf {
                rot: 0.4,
                tint: LEAVES[0],
            },
        );
        g.push(
            vec2(c + 30.0, plaza + 120.0),
            Kind::Leaf {
                rot: 2.2,
                tint: LEAVES[1],
            },
        );
    }

    region_scenery(&mut g, k, seed);

    // The castle's wall (FR27) stands across the road's end: nothing grows past it.
    g.out.retain(|d| d.pos.y > world::CASTLE_WALL_Y);

    if let Some(stop) = stop {
        g.out.retain(|d| !in_stop_box(d.pos, &stop));
        for (pos, _, kind) in stop_layout(&stop) {
            g.push(pos, kind);
        }
    }

    g.out
}

impl Decoration {
    pub fn is_flat(&self) -> bool {
        matches!(
            self.kind,
            Kind::Speckle { .. }
                | Kind::Leaf { .. }
                | Kind::Pawprints { .. }
                | Kind::GrassTuft { .. }
                | Kind::Toadstools { .. }
                | Kind::LilyPad { .. }
                | Kind::WaterRipple { .. }
                | Kind::Wisp { .. }
        )
    }

    /// The pool of light this prop casts on the ground, if any, as
    /// `(world position, radius, colour)`. `World::draw_lights` paints
    /// these under the depth-sorted pass, so the night has warm spots.
    pub fn lights(&self, t: f32, out: &mut Vec<(Vec2, f32, Color)>) {
        let p = self.pos;
        let flicker = |phase: f32| 0.9 + 0.1 * (t * 9.0 + phase).sin() * (t * 5.3 + phase).sin();
        match self.kind {
            Kind::Lamppost { phase } => {
                out.push((p, 74.0 * flicker(phase), gfx::with_alpha(LANTERN, 0.26)))
            }
            Kind::Pumpkin { lit: true, phase } => {
                out.push((p, 34.0 * flicker(phase), gfx::with_alpha(LANTERN, 0.22)))
            }
            Kind::Campfire { phase } => {
                out.push((p, 120.0 * flicker(phase), gfx::with_alpha(LANTERN, 0.32)))
            }
            Kind::Cottage { flip } => {
                let f = if flip { -1.0 } else { 1.0 };
                out.push((
                    p + vec2(f * 14.0, 10.0),
                    46.0,
                    gfx::with_alpha(LANTERN, 0.18),
                ))
            }
            Kind::Cauldron { .. } => out.push((p, 54.0, gfx::with_alpha(LAB_GREEN, 0.20))),
            Kind::Gateway => {
                let half = PATH_HALF_W + 16.0;
                for side in [-1.0, 1.0] {
                    out.push((
                        p + vec2(side * half, 4.0),
                        70.0,
                        gfx::with_alpha(LANTERN, 0.24),
                    ));
                }
            }
            Kind::Wagon => out.push((p + vec2(34.0, 4.0), 52.0, gfx::with_alpha(LANTERN, 0.20))),
            Kind::RestSign | Kind::RegionSign { .. } => {
                out.push((p, 50.0 * flicker(p.y), gfx::with_alpha(LANTERN, 0.20)))
            }
            Kind::Mushroom { cap, .. } if cap == CAP_TEAL => {
                out.push((p, 44.0, gfx::with_alpha(CAP_TEAL, 0.18)))
            }
            _ => {}
        }
    }

    pub fn draw(&self, view: &View, t: f32) {
        let s = view.to_screen(self.pos);
        match self.kind {
            Kind::Speckle { size, dark } => {
                let c = if dark {
                    Color::new(0.04, 0.06, 0.05, 0.55)
                } else {
                    Color::new(0.32, 0.42, 0.28, 0.45)
                };
                draw_rectangle(s.x, s.y, size, size, c);
            }
            Kind::Leaf { rot, tint } => draw_leaf(s, rot, tint),
            Kind::Pawprints { angle } => draw_pawprints(s, angle),
            Kind::GrassTuft { size, phase } => draw_tuft(s, size, phase, t),
            Kind::Toadstools { glow } => draw_toadstools(s, glow, t),
            Kind::LilyPad { rot, flower } => draw_lily_pad(s, rot, flower),
            Kind::WaterRipple { phase } => {
                let u = (t * 0.25 + phase / TAU).fract();
                let a = 0.30 * (1.0 - u);
                gfx::ellipse_outline(
                    s,
                    4.0 + 22.0 * u,
                    1.8 + 8.0 * u,
                    1.2,
                    gfx::with_alpha(MOON, a),
                );
            }
            Kind::Wisp { phase } => draw_wisp(s, phase, t),
            Kind::Pine { height, phase } => draw_pine(s, height, phase, t),
            Kind::DeadTree {
                height,
                lean,
                phase,
            } => draw_dead_tree(s, height, lean, phase, t),
            Kind::Bush { berries } => draw_bush(s, berries),
            Kind::Mushroom { cap, size } => draw_mushroom(s, cap, size),
            Kind::Gravestone { style, tilt } => draw_gravestone(s, style, tilt),
            Kind::Pillar => draw_pillar(s),
            Kind::Cottage { flip } => draw_cottage(s, flip, t),
            Kind::Lamppost { phase } => draw_lamppost(s, phase, t),
            Kind::Pumpkin { lit, phase } => draw_pumpkin(s, lit, phase, t),
            Kind::Cauldron { phase } => draw_cauldron(s, phase, t),
            Kind::Stump { size } => draw_stump(s, size),
            Kind::Rock { size, wet } => draw_rock(s, size, wet, t),
            Kind::Rowboat { phase } => draw_rowboat(s, phase, t),
            Kind::Reeds { phase } => draw_reeds(s, phase, t),
            Kind::Crow { phase } => draw_crow(s, phase, t),
            Kind::Scarecrow { phase } => draw_scarecrow(s, phase, t),
            Kind::Marker { meters } => draw_marker(s, meters),
            Kind::Signpost => draw_signpost(s),
            Kind::Gateway => draw_gateway(s, t),
            Kind::Pebble => draw_pebble(s, t),
            Kind::Campfire { phase } => draw_campfire(s, phase, t),
            Kind::Wagon => draw_wagon(s, t),
            Kind::RestSign => draw_rest_sign(s, t),
            Kind::RegionSign { region } => draw_region_sign(s, region, t),
            Kind::Haystack { flip } => draw_haystack(s, flip),
            Kind::Well => draw_well(s),
        }
    }
}

fn dir(a: f32) -> Vec2 {
    vec2(a.cos(), a.sin())
}

/// The upper half of an ellipse (a cap or a dome), as a triangle fan.
fn dome(c: Vec2, rx: f32, ry: f32, color: Color) {
    const N: usize = 14;
    for i in 0..N {
        let a0 = PI + PI * i as f32 / N as f32;
        let a1 = PI + PI * (i + 1) as f32 / N as f32;
        draw_triangle(
            c,
            c + vec2(a0.cos() * rx, a0.sin() * ry),
            c + vec2(a1.cos() * rx, a1.sin() * ry),
            color,
        );
    }
}

fn dome_outline(c: Vec2, rx: f32, ry: f32, thickness: f32, color: Color) {
    let pts: Vec<Vec2> = (0..=14)
        .map(|i| {
            let a = PI + PI * i as f32 / 14.0;
            c + vec2(a.cos() * rx, a.sin() * ry)
        })
        .collect();
    gfx::polyline(&pts, thickness, color);
    gfx::line(pts[0], pts[14], thickness, color);
}

/// A flame of three nested tongues at `base`, `size` px tall at rest.
fn flame(base: Vec2, size: f32, phase: f32, t: f32) {
    let flick = 0.5 + 0.5 * (t * 11.0 + phase).sin() * (t * 7.3 + phase * 1.7).sin();
    let sway = 1.5 * (t * 5.0 + phase).sin();
    let tongue = |scale: f32, color: Color| {
        let w = size * 0.4 * scale;
        draw_circle(base.x, base.y, w, color);
        draw_triangle(
            base + vec2(-w, 0.0),
            base + vec2(w, 0.0),
            base + vec2(sway * scale, -(size + flick * size * 0.4) * scale),
            color,
        );
    };
    tongue(1.0, Color::new(1.0, 0.42, 0.12, 1.0));
    tongue(0.68, Color::new(1.0, 0.75, 0.20, 1.0));
    tongue(0.36, Color::new(1.0, 0.96, 0.75, 1.0));
}

fn draw_leaf(s: Vec2, rot: f32, tint: Color) {
    let deg = rot.to_degrees();
    gfx::ellipse_rot(s, 4.2, 2.2, deg, tint);
    let d = dir(rot) * 4.0;
    gfx::line(s - d, s + d * 1.3, 0.8, gfx::shade(tint, 0.6));
}

fn draw_pawprints(s: Vec2, angle: f32) {
    let ahead = dir(angle);
    let side = ahead.perp();
    let print = Color::new(0.05, 0.06, 0.05, 0.55);
    for i in 0..6 {
        let foot = if i % 2 == 0 { 1.0 } else { -1.0 };
        let p = s + ahead * (i as f32 * 12.0) + side * (foot * 3.5);
        gfx::ellipse(p, 2.2, 1.6, print);
        for k in -1..=1 {
            let toe = p + ahead * 2.8 + side * (k as f32 * 1.6);
            draw_circle(toe.x, toe.y, 0.8, print);
        }
    }
}

fn draw_tuft(s: Vec2, size: f32, phase: f32, t: f32) {
    let dark = Color::new(0.08, 0.18, 0.11, 1.0);
    let light = Color::new(0.16, 0.30, 0.17, 1.0);
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

fn draw_toadstools(s: Vec2, glow: bool, t: f32) {
    let cap = if glow { CAP_TEAL } else { CAP_RED };
    if glow {
        let pulse = 0.8 + 0.2 * (t * 1.2 + s.x * 0.1).sin();
        gfx::glow(
            s + vec2(0.0, -3.0),
            16.0,
            10.0,
            gfx::with_alpha(cap, 0.30 * pulse),
        );
    }
    for (dx, dy, k) in [(-4.0, 1.0, 0.8f32), (3.0, -1.0, 1.0), (7.0, 2.0, 0.6)] {
        let base = s + vec2(dx, dy);
        gfx::line(base, base + vec2(0.0, -5.0 * k), 1.6 * k, CREAM);
        let c = base + vec2(0.0, -5.0 * k);
        dome(c, 3.6 * k, 3.0 * k, cap);
        draw_circle(
            c.x - 1.0 * k,
            c.y - 1.4 * k,
            0.6 * k,
            gfx::with_alpha(WHITE, 0.8),
        );
    }
}

fn draw_lily_pad(s: Vec2, rot: f32, flower: bool) {
    let green = Color::new(0.18, 0.36, 0.22, 1.0);
    const N: usize = 12;
    for i in 0..N {
        // The notch: one wedge missing.
        if i == 0 {
            continue;
        }
        let a0 = rot + i as f32 / N as f32 * TAU;
        let a1 = rot + (i + 1) as f32 / N as f32 * TAU;
        draw_triangle(
            s,
            s + vec2(a0.cos() * 8.0, a0.sin() * 4.2),
            s + vec2(a1.cos() * 8.0, a1.sin() * 4.2),
            if i % 2 == 0 {
                green
            } else {
                gfx::shade(green, 1.1)
            },
        );
    }
    if flower {
        gfx::star(
            s + vec2(1.0, -2.0),
            4.0,
            1.6,
            6,
            rot,
            0.8,
            Color::new(0.95, 0.75, 0.88, 1.0),
        );
        draw_circle(s.x + 1.0, s.y - 2.0, 1.2, Color::new(1.0, 0.85, 0.35, 1.0));
    }
}

fn draw_wisp(s: Vec2, phase: f32, t: f32) {
    let c = s + vec2(
        10.0 * (t * 0.4 + phase).sin(),
        -14.0 + 4.0 * (t * 0.9 + phase).sin(),
    );
    let pulse = 0.75 + 0.25 * (t * 1.1 + phase).sin();
    let hue = gfx::mix(LAB_GREEN, MOON, 0.5);
    gfx::glow(c, 18.0, 18.0, gfx::with_alpha(hue, 0.35 * pulse));
    // Its reflection on the water.
    gfx::ellipse(
        s + vec2(c.x - s.x, 2.0),
        5.0,
        1.4,
        gfx::with_alpha(hue, 0.25 * pulse),
    );
    draw_circle(c.x, c.y, 3.0, gfx::with_alpha(hue, 0.9));
    draw_circle(c.x, c.y, 1.5, WHITE);
}

fn draw_pine(s: Vec2, h: f32, phase: f32, t: f32) {
    let sway = (t * 0.9 + phase).sin() * 1.5;
    gfx::ground_shadow(s + vec2(h * 0.18, 6.0), 26.0, 8.0);
    let trunk_h = h * 0.16;
    draw_rectangle(s.x - 3.5, s.y - trunk_h, 7.0, trunk_h, BARK);
    draw_rectangle_lines(s.x - 3.5, s.y - trunk_h, 7.0, trunk_h, 1.2, INK);

    let dark = Color::new(0.05, 0.14, 0.11, 1.0);
    let mid = Color::new(0.08, 0.21, 0.15, 1.0);
    let tiers = 4;
    for i in 0..tiers {
        let u = i as f32 / tiers as f32;
        let base_y = s.y - trunk_h - (h - trunk_h) * u * 0.72;
        let top_y = base_y - (h - trunk_h) * 0.42;
        let half = 24.0 * (1.0 - u * 0.62) * (h / 110.0).sqrt();
        let tip = vec2(s.x + sway * (u + 0.3), top_y);
        let l = vec2(s.x - half, base_y);
        let r = vec2(s.x + half, base_y);
        // Ink outline, the shaded right half, the lit left half.
        draw_triangle(
            l + vec2(-1.5, 1.2),
            r + vec2(1.5, 1.2),
            tip + vec2(0.0, -1.8),
            INK,
        );
        draw_triangle(l, r, tip, dark);
        draw_triangle(l, vec2(s.x + half * 0.1, base_y), tip, mid);
        // Ragged lower edge.
        for k in 0..4 {
            let x = l.x + (r.x - l.x) * (k as f32 + 0.5) / 4.0;
            draw_triangle(
                vec2(x - 5.0, base_y - 0.5),
                vec2(x + 5.0, base_y - 0.5),
                vec2(x, base_y + 3.5),
                if k < 2 { mid } else { dark },
            );
        }
        // Moonlight catching the left edge.
        gfx::line(l, tip, 1.2, gfx::with_alpha(MOON, 0.22));
    }
}

fn draw_dead_tree(s: Vec2, h: f32, lean: f32, phase: f32, t: f32) {
    let sway = (t * 0.8 + phase).sin() * 0.03;
    gfx::ground_shadow(s + vec2(10.0, 4.0), 18.0, 5.0);
    let n = 6;
    let pts: Vec<Vec2> = (0..=n)
        .map(|i| {
            let u = i as f32 / n as f32;
            s + vec2(lean * h * u * u + 3.0 * (u * 7.0 + phase).sin() * u, -h * u)
        })
        .collect();
    let widths: Vec<f32> = (0..=n).map(|i| 6.5 - 4.5 * i as f32 / n as f32).collect();
    let outline: Vec<f32> = widths.iter().map(|w| w + 1.3).collect();
    gfx::ribbon(&pts, &outline, INK);
    gfx::ribbon(&pts, &widths, BARK);
    let lit: Vec<Vec2> = pts
        .iter()
        .zip(&widths)
        .map(|(p, w)| *p - vec2(w * 0.5, 0.0))
        .collect();
    let lit_w: Vec<f32> = widths.iter().map(|w| w * 0.3).collect();
    gfx::ribbon(&lit, &lit_w, gfx::mix(BARK, MOON, 0.25));

    // Gnarled bare branches, each with a twig or two.
    for (j, &(at, ang, len)) in [
        (0.45f32, -2.4f32, 0.42f32),
        (0.60, -0.7, 0.38),
        (0.78, -2.0, 0.30),
        (0.88, -1.0, 0.26),
    ]
    .iter()
    .enumerate()
    {
        let i = (at * n as f32) as usize;
        let from = pts[i].lerp(pts[(i + 1).min(n)], at * n as f32 - i as f32);
        let a = ang + sway + 0.25 * (phase + j as f32).sin();
        let l = h * len;
        let mid = from + dir(a) * l * 0.5 + vec2(0.0, -4.0);
        let end = from + dir(a + 0.3) * l;
        let branch = [from, mid, end];
        gfx::ribbon(&branch, &[3.4, 2.2, 0.6], INK);
        gfx::ribbon(&branch, &[2.4, 1.4, 0.3], BARK);
        let twig = mid + dir(a - 0.9) * l * 0.35;
        gfx::line(mid, twig, 1.4, BARK);
    }
    // A knot hole, and sometimes an owl blinking out of it.
    let hole = pts[2] + vec2(0.0, -4.0);
    gfx::ellipse(hole, 3.0, 4.2, Color::new(0.05, 0.03, 0.04, 1.0));
    if phase > 4.6 {
        let blink = (t * 0.5 + phase).fract() < 0.06;
        for dx in [-1.4f32, 1.4] {
            if blink {
                gfx::line(
                    hole + vec2(dx - 0.8, -0.5),
                    hole + vec2(dx + 0.8, -0.5),
                    0.8,
                    LANTERN,
                );
            } else {
                draw_circle(hole.x + dx, hole.y - 0.5, 1.0, LANTERN);
            }
        }
    }
}

fn draw_bush(s: Vec2, berries: Color) {
    gfx::ground_shadow(s + vec2(6.0, 2.0), 22.0, 6.0);
    let dark = Color::new(0.05, 0.15, 0.10, 1.0);
    let mid = Color::new(0.09, 0.23, 0.14, 1.0);
    let light = Color::new(0.15, 0.31, 0.18, 1.0);
    for (dx, dy, r, c) in [
        (-11.0, -9.0, 11.0, dark),
        (10.0, -10.0, 11.0, dark),
        (0.0, -18.0, 12.0, mid),
        (-6.0, -7.0, 9.0, mid),
        (6.0, -6.0, 8.0, light),
        (-4.0, -18.0, 6.0, light),
    ] {
        gfx::ellipse(s + vec2(dx, dy), r, r * 0.8, c);
    }
    for (dx, dy) in [
        (-9.0, -14.0),
        (8.0, -18.0),
        (2.0, -6.0),
        (-2.0, -22.0),
        (12.0, -8.0),
    ] {
        let c = s + vec2(dx, dy);
        draw_circle(c.x, c.y, 2.2, berries);
        draw_circle(c.x - 0.6, c.y - 0.7, 0.7, gfx::with_alpha(WHITE, 0.6));
    }
}

fn draw_mushroom(s: Vec2, cap: Color, k: f32) {
    gfx::ground_shadow(s + vec2(10.0 * k, 3.0), 22.0 * k, 6.0 * k);
    let stalk_h = 24.0 * k;
    let w = 6.0 * k;
    gfx::quad(
        s + vec2(-w, 0.0),
        s + vec2(w, 0.0),
        s + vec2(w * 0.7, -stalk_h),
        s + vec2(-w * 0.7, -stalk_h),
        CREAM,
    );
    gfx::quad(
        s + vec2(-w, 0.0),
        s + vec2(-w * 0.3, 0.0),
        s + vec2(-w * 0.2, -stalk_h),
        s + vec2(-w * 0.7, -stalk_h),
        gfx::shade(CREAM, 1.06),
    );
    gfx::polyline(
        &[
            s + vec2(-w * 0.7, -stalk_h),
            s + vec2(-w, 0.0),
            s + vec2(w, 0.0),
            s + vec2(w * 0.7, -stalk_h),
        ],
        1.2,
        INK,
    );
    let c = s + vec2(0.0, -stalk_h);
    let (rx, ry) = (24.0 * k, 20.0 * k);
    // Gills under the cap, then the dome.
    gfx::ellipse(c, rx, 5.0 * k, gfx::shade(cap, 0.55));
    dome(c + vec2(0.0, -1.0), rx + 1.5, ry + 1.5, INK);
    dome(c + vec2(0.0, -1.0), rx, ry, cap);
    dome(
        c + vec2(-rx * 0.3, -ry * 0.35),
        rx * 0.45,
        ry * 0.45,
        gfx::shade(cap, 1.15),
    );
    for (dx, dy, r) in [
        (-10.0, -10.0, 3.0),
        (6.0, -14.0, 2.6),
        (13.0, -6.0, 2.2),
        (-2.0, -5.0, 2.0),
    ] {
        gfx::ellipse(
            c + vec2(dx, dy) * k,
            r * k,
            r * 0.8 * k,
            gfx::with_alpha(CREAM, 0.9),
        );
    }
}

fn draw_gravestone(s: Vec2, style: u8, tilt: f32) {
    gfx::ground_shadow(s + vec2(7.0, 2.0), 16.0, 4.5);
    let up = vec2(tilt.sin(), -tilt.cos());
    let across = vec2(-up.y, up.x);
    let at = |x: f32, y: f32| s + across * x + up * y;
    let face = STONE;
    let side = gfx::shade(STONE, 0.7);
    match style {
        0 => {
            // A round-topped slab with a carved line or two.
            let (w, h) = (10.0, 20.0);
            gfx::quad(at(-w, 0.0), at(w, 0.0), at(w, h), at(-w, h), side);
            gfx::quad(
                at(-w, 0.0),
                at(w - 2.5, 0.0),
                at(w - 2.5, h),
                at(-w, h),
                face,
            );
            let top = at(-1.25, h);
            dome(top + vec2(0.0, 0.5), w - 1.25, 8.0, face);
            dome_outline(top + vec2(0.0, 0.5), w - 1.25, 8.0, 1.2, INK);
            gfx::polyline(&[at(-w, h), at(-w, 0.0), at(w, 0.0), at(w, h)], 1.2, INK);
            for y in [h * 0.65, h * 0.45] {
                gfx::line(at(-5.5, y), at(3.5, y), 1.0, gfx::shade(face, 0.6));
            }
        }
        1 => {
            // A stone cross.
            gfx::quad(
                at(-3.0, 0.0),
                at(3.0, 0.0),
                at(3.0, 28.0),
                at(-3.0, 28.0),
                face,
            );
            gfx::quad(
                at(-10.0, 17.0),
                at(10.0, 17.0),
                at(10.0, 23.0),
                at(-10.0, 23.0),
                face,
            );
            gfx::polyline(
                &[
                    at(-3.0, 0.0),
                    at(-3.0, 17.0),
                    at(-10.0, 17.0),
                    at(-10.0, 23.0),
                    at(-3.0, 23.0),
                    at(-3.0, 28.0),
                    at(3.0, 28.0),
                    at(3.0, 23.0),
                    at(10.0, 23.0),
                    at(10.0, 17.0),
                    at(3.0, 17.0),
                    at(3.0, 0.0),
                ],
                1.2,
                INK,
            );
        }
        _ => {
            // A little obelisk.
            gfx::quad(
                at(-6.0, 0.0),
                at(6.0, 0.0),
                at(4.0, 26.0),
                at(-4.0, 26.0),
                face,
            );
            gfx::quad(
                at(2.0, 0.0),
                at(6.0, 0.0),
                at(4.0, 26.0),
                at(1.0, 26.0),
                side,
            );
            draw_triangle(at(-4.0, 26.0), at(4.0, 26.0), at(0.0, 32.0), face);
            gfx::polyline(
                &[
                    at(-6.0, 0.0),
                    at(-4.0, 26.0),
                    at(0.0, 32.0),
                    at(4.0, 26.0),
                    at(6.0, 0.0),
                ],
                1.2,
                INK,
            );
        }
    }
    // Moss creeping up the base.
    gfx::ellipse(s + vec2(-3.0, -1.5), 6.0, 2.4, gfx::with_alpha(MOSS, 0.9));
    gfx::ellipse(s + vec2(5.0, -1.0), 4.0, 1.8, gfx::with_alpha(MOSS, 0.8));
}

fn draw_pillar(s: Vec2) {
    gfx::ground_shadow(s + vec2(10.0, 2.0), 24.0, 6.0);
    // Plinth.
    draw_rectangle(s.x - 15.0, s.y - 8.0, 30.0, 8.0, gfx::shade(STONE, 0.85));
    draw_rectangle_lines(s.x - 15.0, s.y - 8.0, 30.0, 8.0, 1.2, INK);
    // The broken column: flutes and a jagged top.
    let (x0, x1, base, top) = (s.x - 10.0, s.x + 10.0, s.y - 8.0, s.y - 40.0);
    let jag = [
        vec2(x0, top + 4.0),
        vec2(x0 + 5.0, top - 2.0),
        vec2(x0 + 9.0, top + 3.0),
        vec2(x0 + 14.0, top - 4.0),
        vec2(x1, top + 1.0),
    ];
    gfx::quad(vec2(x0, base), vec2(x1, base), jag[4], jag[0], STONE);
    for w in jag.windows(2) {
        draw_triangle(w[0], w[1], vec2((w[0].x + w[1].x) * 0.5, top + 6.0), STONE);
    }
    draw_rectangle(
        x1 - 6.0,
        top + 2.0,
        6.0,
        base - top - 2.0,
        gfx::shade(STONE, 0.72),
    );
    for k in 1..4 {
        let x = x0 + k as f32 * 5.0;
        gfx::line(
            vec2(x, base),
            vec2(x, top + 5.0),
            1.0,
            gfx::shade(STONE, 0.75),
        );
    }
    let mut outline = vec![vec2(x0, base)];
    outline.extend_from_slice(&jag);
    outline.push(vec2(x1, base));
    gfx::polyline(&outline, 1.2, INK);
    // The fallen chunk, and moss.
    gfx::ellipse(s + vec2(20.0, -3.0), 8.0, 4.0, gfx::shade(STONE, 0.8));
    gfx::ellipse_outline(s + vec2(20.0, -3.0), 8.0, 4.0, 1.0, INK);
    gfx::ellipse(s + vec2(-6.0, -9.0), 7.0, 2.6, MOSS);
}

fn draw_cottage(s: Vec2, flip: bool, t: f32) {
    let f = if flip { -1.0 } else { 1.0 };
    let wall = Color::new(0.30, 0.24, 0.22, 1.0);
    let slate = Color::new(0.17, 0.15, 0.23, 1.0);
    gfx::ground_shadow(s + vec2(14.0, 3.0), 48.0, 12.0);

    // Crooked walls: the top leans a little.
    let lean = f * 3.0;
    let (bl, br) = (s + vec2(-30.0, 0.0), s + vec2(30.0, 0.0));
    let (tl, tr) = (s + vec2(-30.0 + lean, -38.0), s + vec2(30.0 + lean, -36.0));
    gfx::quad(bl, br, tr, tl, wall);
    gfx::quad(
        bl,
        bl + vec2(7.0, 0.0),
        tl + vec2(7.0, 0.0),
        tl,
        gfx::shade(wall, 1.15),
    );
    // Timber beams.
    for u in [0.33f32, 0.66] {
        gfx::line(bl.lerp(br, u), tl.lerp(tr, u), 2.0, gfx::shade(wall, 0.6));
    }
    gfx::line(
        bl.lerp(tl, 0.5),
        br.lerp(tr, 0.5),
        2.0,
        gfx::shade(wall, 0.6),
    );
    gfx::polyline(&[tl, bl, br, tr], 1.4, INK);
    // Door and a lit window.
    let door_x = s.x - 7.0 - f * 12.0;
    draw_rectangle(
        door_x,
        s.y - 22.0,
        14.0,
        22.0,
        Color::new(0.16, 0.10, 0.08, 1.0),
    );
    dome(
        vec2(door_x + 7.0, s.y - 22.0),
        7.0,
        5.0,
        Color::new(0.16, 0.10, 0.08, 1.0),
    );
    let win = vec2(s.x + f * 13.0 + lean * 0.5, s.y - 24.0);
    let candle = 0.9 + 0.1 * (t * 1.7 + s.x).sin();
    draw_rectangle(
        win.x - 6.0,
        win.y - 6.0,
        12.0,
        11.0,
        gfx::shade(LANTERN, candle),
    );
    gfx::line(
        win + vec2(0.0, -6.0),
        win + vec2(0.0, 5.0),
        1.4,
        gfx::shade(wall, 0.5),
    );
    gfx::line(
        win + vec2(-6.0, -0.5),
        win + vec2(6.0, -0.5),
        1.4,
        gfx::shade(wall, 0.5),
    );
    draw_rectangle_lines(win.x - 6.0, win.y - 6.0, 12.0, 11.0, 1.2, INK);

    // Steep, crooked slate roof with a chimney.
    let roof = [
        tl + vec2(-9.0, 2.0),
        tr + vec2(9.0, 2.0),
        s + vec2(6.0 + lean * 2.0, -76.0),
    ];
    let chim = s + vec2(14.0 * f + lean, -58.0);
    draw_rectangle(
        chim.x - 4.0,
        chim.y - 14.0,
        8.0,
        16.0,
        gfx::shade(wall, 0.8),
    );
    draw_rectangle_lines(chim.x - 4.0, chim.y - 14.0, 8.0, 16.0, 1.2, INK);
    for i in 0..3 {
        let u = (t * 0.25 + i as f32 / 3.0 + s.x * 0.01).fract();
        let p = chim
            + vec2(
                4.0 * (u * 5.0 + i as f32).sin() + u * 10.0,
                -16.0 - u * 26.0,
            );
        draw_circle(
            p.x,
            p.y,
            3.0 + u * 4.0,
            gfx::with_alpha(MOON, 0.22 * (1.0 - u)),
        );
    }
    draw_triangle(roof[0], roof[1], roof[2], slate);
    draw_triangle(
        roof[0],
        roof[0].lerp(roof[1], 0.4),
        roof[2],
        gfx::shade(slate, 1.2),
    );
    for u in [0.3f32, 0.55, 0.8] {
        gfx::line(
            roof[0].lerp(roof[2], u),
            roof[1].lerp(roof[2], u),
            1.0,
            gfx::shade(slate, 0.7),
        );
    }
    gfx::polyline(&[roof[0], roof[2], roof[1], roof[0]], 1.4, INK);
}

fn draw_lamppost(s: Vec2, phase: f32, t: f32) {
    let h = 46.0;
    let top = s + vec2(0.0, -h);
    gfx::ground_shadow(s + vec2(4.0, 1.0), 6.0, 2.5);
    draw_rectangle(s.x - 5.0, s.y - 5.0, 10.0, 5.0, IRON);
    draw_rectangle(s.x - 2.0, top.y, 4.0, h, IRON);
    draw_rectangle(s.x - 2.0, top.y, 1.2, h, gfx::with_alpha(MOON, 0.35));
    draw_rectangle_lines(s.x - 5.0, s.y - 5.0, 10.0, 5.0, 1.0, INK);
    // A curled arm with the lantern hanging from it.
    let arm = [top, top + vec2(6.0, -6.0), top + vec2(13.0, -4.0)];
    gfx::polyline(&arm, 2.0, IRON);
    let hook = top + vec2(13.0, -4.0);
    let swing = 1.2 * (t * 1.8 + phase).sin();
    let lamp = hook + vec2(swing, 9.0);
    gfx::line(hook, lamp + vec2(0.0, -5.0), 1.0, IRON);
    let flick = 0.85 + 0.15 * (t * 9.0 + phase).sin() * (t * 5.3 + phase).sin();
    gfx::glow(lamp, 20.0, 20.0, gfx::with_alpha(LANTERN, 0.35 * flick));
    draw_rectangle(
        lamp.x - 4.0,
        lamp.y - 5.0,
        8.0,
        9.0,
        gfx::shade(LANTERN, flick),
    );
    draw_rectangle_lines(lamp.x - 4.0, lamp.y - 5.0, 8.0, 9.0, 1.2, IRON);
    draw_triangle(
        lamp + vec2(-5.5, -5.0),
        lamp + vec2(5.5, -5.0),
        lamp + vec2(0.0, -9.0),
        IRON,
    );
    draw_circle(lamp.x, lamp.y, 1.6, Color::new(1.0, 0.97, 0.80, 1.0));
}

fn draw_pumpkin(s: Vec2, lit: bool, phase: f32, t: f32) {
    gfx::ground_shadow(s + vec2(4.0, 1.0), 12.0, 3.5);
    let c = s + vec2(0.0, -8.0);
    gfx::ellipse(c, 12.5, 9.5, INK);
    for (dx, rx, k) in [(-5.5, 6.5, 0.85f32), (5.5, 6.5, 0.8), (0.0, 7.0, 1.0)] {
        gfx::ellipse(c + vec2(dx, 0.0), rx, 8.5, gfx::shade(PUMPKIN, k));
    }
    gfx::ellipse(c + vec2(-3.0, -4.0), 3.0, 1.6, gfx::with_alpha(WHITE, 0.25));
    gfx::line(
        c + vec2(0.0, -8.0),
        c + vec2(2.0, -13.0),
        2.4,
        Color::new(0.25, 0.40, 0.18, 1.0),
    );
    if lit {
        let flick = 0.8 + 0.2 * (t * 9.0 + phase).sin() * (t * 5.3 + phase).sin();
        let fire = gfx::shade(Color::new(1.0, 0.86, 0.35, 1.0), flick);
        // A friendly carved face.
        for dx in [-4.5f32, 4.5] {
            draw_triangle(
                c + vec2(dx - 2.4, -0.5),
                c + vec2(dx + 2.4, -0.5),
                c + vec2(dx, -4.5),
                fire,
            );
        }
        let mouth: Vec<Vec2> = (0..=6)
            .map(|i| {
                let x = -6.0 + i as f32 * 2.0;
                c + vec2(x, 3.0 + 0.06 * x * x + if i % 2 == 0 { 0.0 } else { 1.2 })
            })
            .collect();
        gfx::polyline(&mouth, 2.4, fire);
    }
}

fn draw_cauldron(s: Vec2, phase: f32, t: f32) {
    gfx::ground_shadow(s + vec2(4.0, 2.0), 16.0, 4.5);
    flame(s + vec2(0.0, -1.0), 7.0, phase, t);
    let c = s + vec2(0.0, -13.0);
    for dx in [-9.0f32, 9.0] {
        gfx::line(c + vec2(dx, 4.0), s + vec2(dx * 1.2, 0.0), 2.2, IRON);
    }
    gfx::ellipse(c, 15.0, 11.0, INK);
    gfx::ellipse(c, 13.5, 10.0, IRON);
    gfx::ellipse(c + vec2(-5.0, -1.0), 3.0, 5.0, gfx::with_alpha(MOON, 0.18));
    // The rim and the bubbling brew.
    let rim = c + vec2(0.0, -7.0);
    gfx::ellipse(rim, 13.0, 4.0, gfx::shade(IRON, 1.6));
    gfx::ellipse(rim, 11.0, 3.0, gfx::shade(LAB_GREEN, 0.75));
    gfx::glow(rim, 22.0, 14.0, gfx::with_alpha(LAB_GREEN, 0.25));
    for i in 0..4 {
        let u = (t * 0.8 + i as f32 * 0.27 + phase).fract();
        let p = rim + vec2((i as f32 - 1.5) * 5.0, -u * 22.0);
        draw_circle(
            p.x + 2.0 * (u * 6.0 + i as f32).sin(),
            p.y,
            1.5 + u * 2.5,
            gfx::with_alpha(LAB_GREEN, 0.55 * (1.0 - u)),
        );
    }
}

fn draw_stump(s: Vec2, size: f32) {
    gfx::ground_shadow(s + vec2(5.0, 1.5), size * 1.3, size * 0.4);
    let h = size * 0.9;
    draw_rectangle(s.x - size, s.y - h, size * 2.0, h, BARK);
    draw_rectangle(s.x - size, s.y - h, size * 0.6, h, gfx::shade(BARK, 1.2));
    draw_rectangle_lines(s.x - size, s.y - h, size * 2.0, h, 1.2, INK);
    let top = vec2(s.x, s.y - h);
    let wood = Color::new(0.62, 0.48, 0.34, 1.0);
    gfx::ellipse(top, size, size * 0.4, wood);
    gfx::ellipse_outline(top, size * 0.6, size * 0.24, 0.8, gfx::shade(wood, 0.7));
    gfx::ellipse_outline(top, size, size * 0.4, 1.2, INK);
}

fn draw_rock(s: Vec2, size: f32, wet: bool, t: f32) {
    let base = if wet {
        Color::new(0.20, 0.22, 0.26, 1.0)
    } else {
        Color::new(0.36, 0.37, 0.42, 1.0)
    };
    if wet {
        let p = 1.0 + 0.10 * (t * 1.2 + s.x * 0.05).sin();
        gfx::ellipse_outline(
            s,
            size * 1.4 * p,
            size * 0.5 * p,
            1.4,
            gfx::with_alpha(MOON, 0.25),
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
    if !wet {
        gfx::ellipse(
            s + vec2(-size * 0.2, -size * 0.75),
            size * 0.5,
            size * 0.18,
            MOSS,
        );
    }
    gfx::ellipse(
        s + vec2(-size * 0.45, -size * 0.6),
        size * 0.22,
        size * 0.1,
        gfx::with_alpha(MOON, 0.45),
    );
    gfx::ellipse_outline(body, size, size * 0.62, 1.2, gfx::with_alpha(INK, 0.55));
}

fn draw_rowboat(s: Vec2, phase: f32, t: f32) {
    let c = s + vec2(
        14.0 * (t * 0.08 + phase).sin(),
        1.2 * (t * 1.1 + phase).sin(),
    );
    let rock = 0.04 * (t * 1.1 + phase).sin();
    let r = |v: Vec2| c + vec2(v.x - v.y * rock, v.y + v.x * rock);
    gfx::ellipse_outline(
        c + vec2(0.0, 4.0),
        30.0,
        6.5,
        1.3,
        gfx::with_alpha(MOON, 0.25),
    );
    let hull = Color::new(0.30, 0.20, 0.15, 1.0);
    // Hull from the side: a curved bottom, pointed bow on the left.
    let rim = [vec2(-26.0, -8.0), vec2(22.0, -8.0)];
    let bottom: Vec<Vec2> = (0..=8)
        .map(|i| {
            let u = i as f32 / 8.0;
            vec2(-26.0 + 48.0 * u, -8.0 + 10.0 * (u * PI).sin().powf(0.6))
        })
        .collect();
    for w in bottom.windows(2) {
        gfx::quad(
            r(vec2(w[0].x, -8.0)),
            r(vec2(w[1].x, -8.0)),
            r(w[1]),
            r(w[0]),
            hull,
        );
    }
    gfx::line(
        r(vec2(-24.0, -3.0)),
        r(vec2(20.0, -3.0)),
        1.0,
        gfx::shade(hull, 0.7),
    );
    gfx::line(r(rim[0]), r(rim[1]), 2.2, gfx::shade(hull, 1.4));
    let mut outline: Vec<Vec2> = bottom.iter().map(|&p| r(p)).collect();
    outline.push(r(rim[1]));
    outline.insert(0, r(rim[0]));
    gfx::polyline(&outline, 1.2, INK);
    // An oar resting across it and a little lantern on the bow.
    gfx::line(r(vec2(-8.0, -12.0)), r(vec2(30.0, 2.0)), 2.0, WOOD);
    let lamp = r(vec2(-22.0, -14.0));
    gfx::glow(lamp, 16.0, 16.0, gfx::with_alpha(LANTERN, 0.35));
    draw_rectangle(lamp.x - 2.5, lamp.y - 3.0, 5.0, 6.0, LANTERN);
    draw_rectangle_lines(lamp.x - 2.5, lamp.y - 3.0, 5.0, 6.0, 1.0, IRON);
}

fn draw_reeds(s: Vec2, phase: f32, t: f32) {
    let stalk = Color::new(0.20, 0.30, 0.17, 1.0);
    let head = Color::new(0.36, 0.22, 0.14, 1.0);
    for i in 0..5 {
        let fi = i as f32 - 2.0;
        let sway = 2.5 * (t * 1.3 + phase + fi * 0.7).sin();
        let base = s + vec2(fi * 3.5, 0.0);
        let h = 26.0 + 8.0 * ((i * 5 % 3) as f32);
        let tip = base + vec2(sway + fi * 1.5, -h);
        gfx::line(base, tip, 1.4, stalk);
        if i % 2 == 0 {
            let c = base.lerp(tip, 0.78);
            gfx::ellipse_rot(c, 1.9, 5.0, (sway * 2.0).to_degrees() * 0.05, head);
        } else {
            gfx::ribbon(
                &[
                    base,
                    base.lerp(tip, 0.5) + vec2(fi * 2.0, 0.0),
                    tip + vec2(fi * 3.0, 6.0),
                ],
                &[1.6, 1.0, 0.2],
                gfx::shade(stalk, 1.25),
            );
        }
    }
}

fn draw_crow(s: Vec2, phase: f32, t: f32) {
    let hop = (t * 2.0 + phase).sin().max(0.0);
    let c = s + vec2(4.0 * (t * 0.3 + phase).sin(), -8.0 - hop * 5.0);
    let face = if (t * 0.15 + phase).sin() > 0.0 {
        1.0
    } else {
        -1.0
    };
    let black = Color::new(0.07, 0.06, 0.10, 1.0);
    gfx::ground_shadow(s + vec2(2.0, 1.0), 6.0 - hop * 2.0, 2.0);
    for dx in [-1.5f32, 1.5] {
        gfx::line(
            c + vec2(dx, 3.0),
            s + vec2(dx + (c.x - s.x), 0.0),
            1.0,
            LANTERN,
        );
    }
    draw_triangle(
        c + vec2(-face * 5.0, -1.0),
        c + vec2(-face * 12.0, -3.0),
        c + vec2(-face * 11.0, 2.0),
        black,
    );
    gfx::ellipse(c, 6.5, 4.8, black);
    gfx::ellipse(
        c + vec2(-face * 1.0, -1.0),
        4.0,
        2.4,
        gfx::shade(black, 1.8),
    );
    let head = c + vec2(face * 5.0, -4.0);
    draw_circle(head.x, head.y, 3.6, black);
    draw_triangle(
        head + vec2(face * 2.5, -1.2),
        head + vec2(face * 2.5, 1.2),
        head + vec2(face * 6.5, 0.4),
        Color::new(0.95, 0.70, 0.25, 1.0),
    );
    draw_circle(head.x + face * 1.2, head.y - 0.8, 1.0, WHITE);
}

fn draw_scarecrow(s: Vec2, phase: f32, t: f32) {
    let sway = 0.05 * (t * 0.9 + phase).sin();
    let up = vec2(sway, -1.0);
    gfx::ground_shadow(s + vec2(10.0, 3.0), 18.0, 5.0);
    let top = s + up * 58.0;
    gfx::line(s, top, 4.0, WOOD);
    let arm_y = s + up * 40.0;
    gfx::line(arm_y + vec2(-22.0, 0.0), arm_y + vec2(22.0, 0.0), 3.4, WOOD);
    // A patched purple coat with straw poking out.
    let coat = Color::new(0.38, 0.24, 0.48, 1.0);
    gfx::quad(
        arm_y + vec2(-18.0, -3.0),
        arm_y + vec2(18.0, -3.0),
        arm_y + vec2(10.0, 24.0),
        arm_y + vec2(-10.0, 24.0),
        coat,
    );
    gfx::polyline(
        &[
            arm_y + vec2(-18.0, -3.0),
            arm_y + vec2(-10.0, 24.0),
            arm_y + vec2(10.0, 24.0),
            arm_y + vec2(18.0, -3.0),
        ],
        1.2,
        INK,
    );
    draw_rectangle(
        arm_y.x + 2.0,
        arm_y.y + 6.0,
        6.0,
        6.0,
        Color::new(0.70, 0.55, 0.25, 1.0),
    );
    let straw = Color::new(0.85, 0.72, 0.36, 1.0);
    for side in [-1.0f32, 1.0] {
        for k in 0..3 {
            let a = arm_y + vec2(side * 22.0, 0.0);
            gfx::line(a, a + vec2(side * 5.0, -3.0 + k as f32 * 3.0), 1.2, straw);
        }
    }
    // Sack head with a stitched smile, and a floppy pointed hat.
    let head = top + vec2(0.0, 4.0);
    draw_circle(head.x, head.y, 9.0, Color::new(0.80, 0.68, 0.46, 1.0));
    draw_circle_lines(head.x, head.y, 9.0, 1.2, INK);
    for dx in [-3.5f32, 3.5] {
        gfx::line(
            head + vec2(dx - 1.5, -2.5),
            head + vec2(dx + 1.5, 0.5),
            1.2,
            INK,
        );
        gfx::line(
            head + vec2(dx + 1.5, -2.5),
            head + vec2(dx - 1.5, 0.5),
            1.2,
            INK,
        );
    }
    let smile: Vec<Vec2> = (0..=6)
        .map(|i| {
            let x = -5.0 + i as f32 * 10.0 / 6.0;
            head + vec2(x, 4.0 + 0.08 * (25.0 - x * x).max(0.0) * 0.3)
        })
        .collect();
    gfx::polyline(&smile, 1.1, INK);
    let brim = head + vec2(0.0, -6.0);
    gfx::ellipse(brim, 14.0, 3.0, Color::new(0.20, 0.16, 0.22, 1.0));
    draw_triangle(
        brim + vec2(-8.0, -1.0),
        brim + vec2(8.0, -1.0),
        brim + vec2(10.0 + 3.0 * (t + phase).sin(), -18.0),
        Color::new(0.20, 0.16, 0.22, 1.0),
    );
}

fn stone_slab(x: f32, y: f32, w: f32, h: f32) {
    let face = Color::new(0.64, 0.64, 0.68, 1.0);
    draw_rectangle(x, y, w, h, face);
    draw_rectangle(x, y, w, 2.5, gfx::shade(face, 1.15));
    draw_rectangle_lines(x, y, w, h, 1.5, INK);
}

fn wood_board(x: f32, y: f32, w: f32, h: f32) {
    let board = Color::new(0.70, 0.54, 0.36, 1.0);
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

const CARVED: Color = Color::new(0.16, 0.12, 0.14, 1.0);

/// A distance milestone: a pale stone with the metres carved in it, pale
/// so it reads against the dark ground.
fn draw_marker(s: Vec2, meters: u32) {
    gfx::ground_shadow(s + vec2(5.0, 1.0), 14.0, 3.0);
    stone_slab(s.x - 17.0, s.y - 24.0, 34.0, 24.0);
    dome(
        vec2(s.x, s.y - 24.0),
        17.0,
        6.0,
        Color::new(0.64, 0.64, 0.68, 1.0),
    );
    dome_outline(vec2(s.x, s.y - 24.0), 17.0, 6.0, 1.5, INK);
    centered_text(&format!("{meters}m"), s.x, s.y - 8.0, 16, CARVED);
    gfx::ellipse(s + vec2(-8.0, -1.5), 7.0, 2.2, MOSS);
}

fn draw_signpost(s: Vec2) {
    gfx::ground_shadow(s + vec2(8.0, 2.0), 44.0, 6.0);
    for dx in [-32.0, 32.0] {
        draw_rectangle(s.x + dx - 2.5, s.y - 34.0, 5.0, 34.0, WOOD);
        draw_rectangle_lines(s.x + dx - 2.5, s.y - 34.0, 5.0, 34.0, 1.0, INK);
    }
    wood_board(s.x - 50.0, s.y - 60.0, 100.0, 30.0);
    centered_text("JEKO WOODS", s.x - 8.0, s.y - 40.0, 18, CARVED);
    // Painted arrow pointing up the road, towards the manor.
    let a = vec2(s.x + 38.0, s.y - 45.0);
    draw_rectangle(a.x - 2.0, a.y - 2.0, 4.0, 10.0, CAP_RED);
    draw_triangle(
        a + vec2(-6.5, -1.0),
        a + vec2(6.5, -1.0),
        a + vec2(0.0, -10.0),
        CAP_RED,
    );
}

/// The sign at a safe stop (FR28): a board hung from a post with a lantern,
/// reading "REST".
fn draw_rest_sign(s: Vec2, t: f32) {
    gfx::ground_shadow(s + vec2(5.0, 2.0), 22.0, 5.0);
    draw_rectangle(s.x - 2.5, s.y - 56.0, 5.0, 56.0, WOOD);
    draw_rectangle_lines(s.x - 2.5, s.y - 56.0, 5.0, 56.0, 1.0, INK);
    wood_board(s.x - 26.0, s.y - 54.0, 52.0, 22.0);
    centered_text("REST", s.x, s.y - 38.0, 18, CARVED);
    // The lantern swinging from the arm.
    let lamp = s + vec2(20.0 + 1.2 * (t * 1.7 + s.y).sin(), -22.0);
    gfx::line(s + vec2(0.0, -26.0), lamp + vec2(0.0, -4.0), 1.0, IRON);
    gfx::glow(lamp, 14.0, 14.0, gfx::with_alpha(LANTERN, 0.35));
    draw_rectangle(lamp.x - 3.0, lamp.y - 4.0, 6.0, 7.0, LANTERN);
    draw_rectangle_lines(lamp.x - 3.0, lamp.y - 4.0, 6.0, 7.0, 1.0, IRON);
}

/// The sign at a region's border (FR27): a board on two posts with the name
/// under "ENTERING", and a lantern, so it reads from down the road.
fn draw_region_sign(s: Vec2, region: u8, t: f32) {
    let name = regions::REGIONS.get(region as usize).map_or("", |r| r.name);
    let w = measure_text(name, None, 15, 1.0).width.max(70.0) + 18.0;
    gfx::ground_shadow(s + vec2(5.0, 2.0), w * 0.5, 5.0);
    for dx in [-w * 0.5 + 6.0, w * 0.5 - 6.0] {
        draw_rectangle(s.x + dx - 2.5, s.y - 52.0, 5.0, 52.0, WOOD);
        draw_rectangle_lines(s.x + dx - 2.5, s.y - 52.0, 5.0, 52.0, 1.0, INK);
    }
    wood_board(s.x - w * 0.5, s.y - 56.0, w, 34.0);
    centered_text("ENTERING", s.x, s.y - 43.0, 12, CARVED);
    centered_text(name, s.x, s.y - 28.0, 15, CARVED);
    // A lantern swinging under the board.
    let lamp = s + vec2(w * 0.5 - 8.0, -14.0 + 1.0 * (t * 1.7 + s.y).sin());
    gfx::glow(lamp, 12.0, 12.0, gfx::with_alpha(LANTERN, 0.35));
    draw_rectangle(lamp.x - 3.0, lamp.y - 4.0, 6.0, 7.0, LANTERN);
    draw_rectangle_lines(lamp.x - 3.0, lamp.y - 4.0, 6.0, 7.0, 1.0, IRON);
}

/// A round haystack with a few loose straws.
fn draw_haystack(s: Vec2, flip: bool) {
    let straw = Color::new(0.78, 0.64, 0.30, 1.0);
    let f = if flip { -1.0 } else { 1.0 };
    gfx::ground_shadow(s + vec2(8.0, 2.0), 26.0, 6.0);
    gfx::ellipse(s + vec2(0.0, -2.0), 22.0, 8.0, gfx::shade(straw, 0.8));
    draw_rectangle(s.x - 20.0, s.y - 24.0, 40.0, 22.0, straw);
    dome(vec2(s.x, s.y - 24.0), 20.0, 13.0, gfx::shade(straw, 1.08));
    dome_outline(vec2(s.x, s.y - 24.0), 20.0, 13.0, 1.5, INK);
    draw_rectangle_lines(s.x - 20.0, s.y - 24.0, 40.0, 22.0, 1.5, INK);
    for i in 0..4 {
        let x = s.x - 14.0 + i as f32 * 9.0;
        gfx::line(
            vec2(x, s.y - 20.0),
            vec2(x + f * 2.0, s.y - 6.0),
            1.0,
            gfx::shade(straw, 0.7),
        );
    }
    // A fork left standing in it.
    gfx::line(
        s + vec2(f * 14.0, -2.0),
        s + vec2(f * 22.0, -34.0),
        2.0,
        WOOD,
    );
}

/// A stone well with a little roof, a rope and a bucket.
fn draw_well(s: Vec2) {
    gfx::ground_shadow(s + vec2(6.0, 2.0), 24.0, 6.0);
    // The round wall.
    gfx::ellipse(
        s + vec2(0.0, -2.0),
        19.0,
        8.0,
        Color::new(0.50, 0.50, 0.55, 1.0),
    );
    draw_rectangle(
        s.x - 19.0,
        s.y - 18.0,
        38.0,
        16.0,
        Color::new(0.58, 0.58, 0.62, 1.0),
    );
    gfx::ellipse(
        s + vec2(0.0, -18.0),
        19.0,
        8.0,
        Color::new(0.66, 0.66, 0.70, 1.0),
    );
    gfx::ellipse(
        s + vec2(0.0, -18.0),
        13.0,
        5.0,
        Color::new(0.02, 0.04, 0.08, 1.0),
    );
    for i in 0..3 {
        let y = s.y - 14.0 + i as f32 * 5.0;
        gfx::line(
            vec2(s.x - 19.0, y),
            vec2(s.x + 19.0, y),
            1.0,
            gfx::with_alpha(INK, 0.5),
        );
    }
    draw_rectangle_lines(s.x - 19.0, s.y - 18.0, 38.0, 16.0, 1.5, INK);
    // Two posts, a beam and a pitched roof.
    for dx in [-17.0f32, 17.0] {
        draw_rectangle(s.x + dx - 2.0, s.y - 52.0, 4.0, 36.0, WOOD);
        draw_rectangle_lines(s.x + dx - 2.0, s.y - 52.0, 4.0, 36.0, 1.0, INK);
    }
    draw_triangle(
        vec2(s.x - 25.0, s.y - 50.0),
        vec2(s.x + 25.0, s.y - 50.0),
        vec2(s.x, s.y - 68.0),
        Color::new(0.34, 0.20, 0.18, 1.0),
    );
    draw_line(s.x - 25.0, s.y - 50.0, s.x, s.y - 68.0, 1.2, INK);
    draw_line(s.x + 25.0, s.y - 50.0, s.x, s.y - 68.0, 1.2, INK);
    // Rope and bucket.
    gfx::line(vec2(s.x, s.y - 50.0), vec2(s.x, s.y - 34.0), 1.0, INK);
    draw_rectangle(s.x - 4.0, s.y - 34.0, 8.0, 7.0, WOOD);
    draw_rectangle_lines(s.x - 4.0, s.y - 34.0, 8.0, 7.0, 1.0, INK);
}

/// The hub's gate: two stone pillars with lanterns either side of the
/// road's south end, an iron arch with a sign between them, and the gates
/// swung open. The player walks under it.
fn draw_gateway(s: Vec2, t: f32) {
    let half = PATH_HALF_W + 16.0;
    let top = 92.0;
    // The open gate leaves, folded back against the wall line.
    for side in [-1.0f32, 1.0] {
        let hinge = s.x + side * (half - 10.0);
        for k in 0..5 {
            let x = hinge - side * k as f32 * 6.0;
            gfx::line(vec2(x, s.y - 2.0), vec2(x, s.y - 46.0), 1.6, IRON);
            draw_triangle(
                vec2(x - 2.0, s.y - 46.0),
                vec2(x + 2.0, s.y - 46.0),
                vec2(x, s.y - 52.0),
                IRON,
            );
        }
        gfx::line(
            vec2(hinge, s.y - 10.0),
            vec2(hinge - side * 26.0, s.y - 10.0),
            2.0,
            IRON,
        );
        gfx::line(
            vec2(hinge, s.y - 38.0),
            vec2(hinge - side * 26.0, s.y - 38.0),
            2.0,
            IRON,
        );
    }
    for side in [-1.0f32, 1.0] {
        let x = s.x + side * half;
        gfx::ground_shadow(vec2(x + 8.0, s.y + 2.0), 16.0, 4.5);
        draw_rectangle(x - 9.0, s.y - top, 18.0, top, STONE);
        draw_rectangle(x - 9.0, s.y - top, 5.0, top, gfx::shade(STONE, 1.15));
        for k in 1..6 {
            let y = s.y - top + k as f32 * top / 6.0;
            gfx::line(
                vec2(x - 9.0, y),
                vec2(x + 9.0, y),
                1.0,
                gfx::shade(STONE, 0.7),
            );
        }
        draw_rectangle_lines(x - 9.0, s.y - top, 18.0, top, 1.5, INK);
        // Cap stone and a lantern on top.
        draw_rectangle(x - 12.0, s.y - top - 6.0, 24.0, 6.0, gfx::shade(STONE, 1.1));
        draw_rectangle_lines(x - 12.0, s.y - top - 6.0, 24.0, 6.0, 1.5, INK);
        let lamp = vec2(x, s.y - top - 14.0);
        let flick = 0.85 + 0.15 * (t * 9.0 + side * 2.0).sin() * (t * 5.3 + side).sin();
        gfx::glow(lamp, 26.0, 26.0, gfx::with_alpha(LANTERN, 0.35 * flick));
        draw_rectangle(
            lamp.x - 5.0,
            lamp.y - 6.0,
            10.0,
            12.0,
            gfx::shade(LANTERN, flick),
        );
        draw_rectangle_lines(lamp.x - 5.0, lamp.y - 6.0, 10.0, 12.0, 1.5, IRON);
        draw_triangle(
            lamp + vec2(-7.0, -6.0),
            lamp + vec2(7.0, -6.0),
            lamp + vec2(0.0, -12.0),
            IRON,
        );
    }
    // The wrought-iron arch between the pillars, with curls and spikes.
    let arch: Vec<Vec2> = (0..=16)
        .map(|i| {
            let u = i as f32 / 16.0;
            vec2(
                s.x - half + 9.0 + (half * 2.0 - 18.0) * u,
                s.y - top + 8.0 - 22.0 * (u * PI).sin(),
            )
        })
        .collect();
    gfx::polyline(&arch, 4.0, INK);
    gfx::polyline(&arch, 2.6, IRON);
    for (i, p) in arch.iter().enumerate().filter(|(i, _)| i % 2 == 0) {
        if i == 0 || i == 16 {
            continue;
        }
        draw_triangle(
            *p + vec2(-2.0, 0.0),
            *p + vec2(2.0, 0.0),
            *p + vec2(0.0, -7.0),
            IRON,
        );
    }
    let lower: Vec<Vec2> = arch.iter().map(|p| *p + vec2(0.0, 10.0)).collect();
    gfx::polyline(&lower, 2.0, IRON);
    for p in arch.iter().step_by(4).skip(1).take(3) {
        draw_circle_lines(p.x, p.y + 5.0, 3.2, 1.4, IRON);
    }
    wood_board(s.x - 56.0, s.y - top + 20.0, 112.0, 24.0);
    centered_text("THE ROAD", s.x - 8.0, s.y - top + 38.0, 18, CARVED);
    let a = vec2(s.x + 40.0, s.y - top + 33.0);
    draw_rectangle(a.x - 2.0, a.y - 2.0, 4.0, 9.0, CAP_RED);
    draw_triangle(
        a + vec2(-6.0, -1.0),
        a + vec2(6.0, -1.0),
        a + vec2(0.0, -9.0),
        CAP_RED,
    );
}

/// Professor Pebble: a round grey river pebble with spectacles, a bushy
/// moustache and a tiny mortarboard, bobbing on stubby legs.
fn draw_pebble(s: Vec2, t: f32) {
    let stone = Color::new(0.72, 0.72, 0.74, 1.0);
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
    gfx::line(top, top + vec2(9.0 + swing, 7.0), 1.2, LANTERN);
}

fn draw_campfire(s: Vec2, phase: f32, t: f32) {
    // A ring of stones, two crossed logs, the fire and its sparks.
    for i in 0..8 {
        let a = i as f32 / 8.0 * TAU;
        let p = s + vec2(a.cos() * 15.0, a.sin() * 6.0);
        gfx::ellipse(p, 4.5, 3.0, gfx::shade(STONE, 0.8));
        gfx::ellipse_outline(p, 4.5, 3.0, 1.0, gfx::with_alpha(INK, 0.6));
    }
    for (a, b) in [
        (vec2(-11.0, -1.0), vec2(9.0, -5.0)),
        (vec2(-9.0, -5.0), vec2(11.0, -1.0)),
    ] {
        gfx::line(s + a, s + b, 5.0, INK);
        gfx::line(s + a, s + b, 3.6, BARK);
    }
    flame(s + vec2(0.0, -5.0), 13.0, phase, t);
    for i in 0..5 {
        let u = (t * 0.6 + i as f32 * 0.21 + phase).fract();
        let p = s + vec2(6.0 * (u * 7.0 + i as f32 * 2.0).sin(), -14.0 - u * 40.0);
        draw_circle(
            p.x,
            p.y,
            1.2 * (1.0 - u) + 0.3,
            gfx::with_alpha(LANTERN, 1.0 - u),
        );
    }
}

fn draw_wagon(s: Vec2, t: f32) {
    gfx::ground_shadow(s + vec2(10.0, 4.0), 44.0, 9.0);
    let bed = Color::new(0.42, 0.28, 0.18, 1.0);
    // Back wheels, the bed, the canvas cover, then the front wheels.
    let wheel = |c: Vec2, r: f32| {
        draw_circle(c.x, c.y, r + 1.4, INK);
        draw_circle(c.x, c.y, r, gfx::shade(WOOD, 1.2));
        draw_circle(c.x, c.y, r - 2.5, gfx::shade(WOOD, 0.6));
        for k in 0..6 {
            let a = k as f32 / 6.0 * PI;
            gfx::line(
                c - dir(a) * (r - 2.5),
                c + dir(a) * (r - 2.5),
                1.4,
                gfx::shade(WOOD, 1.2),
            );
        }
        draw_circle(c.x, c.y, 2.2, IRON);
    };
    draw_rectangle(s.x - 34.0, s.y - 26.0, 68.0, 12.0, bed);
    draw_rectangle(s.x - 34.0, s.y - 26.0, 68.0, 3.0, gfx::shade(bed, 1.25));
    draw_rectangle_lines(s.x - 34.0, s.y - 26.0, 68.0, 12.0, 1.4, INK);
    let canvas = Color::new(0.85, 0.80, 0.68, 1.0);
    let hoop_c = vec2(s.x - 2.0, s.y - 26.0);
    dome(hoop_c, 34.0, 30.0, canvas);
    dome(
        hoop_c + vec2(-10.0, -6.0),
        14.0,
        16.0,
        gfx::shade(canvas, 1.06),
    );
    for dx in [-16.0f32, 0.0, 16.0] {
        let h = 30.0 * (1.0 - (dx / 34.0).powi(2)).sqrt();
        gfx::line(
            hoop_c + vec2(dx, 0.0),
            hoop_c + vec2(dx, -h),
            1.2,
            gfx::shade(canvas, 0.75),
        );
    }
    dome_outline(hoop_c, 34.0, 30.0, 1.4, INK);
    // A patch, and the lantern hung at the front.
    draw_rectangle(
        s.x + 6.0,
        s.y - 44.0,
        9.0,
        8.0,
        Color::new(0.48, 0.30, 0.52, 1.0),
    );
    let lamp = s + vec2(36.0 + 1.0 * (t * 1.5).sin(), -30.0);
    gfx::line(s + vec2(32.0, -40.0), lamp + vec2(0.0, -4.0), 1.0, IRON);
    gfx::glow(lamp, 18.0, 18.0, gfx::with_alpha(LANTERN, 0.35));
    draw_rectangle(lamp.x - 3.0, lamp.y - 4.0, 6.0, 7.0, LANTERN);
    draw_rectangle_lines(lamp.x - 3.0, lamp.y - 4.0, 6.0, 7.0, 1.0, IRON);
    wheel(s + vec2(-22.0, -8.0), 10.0);
    wheel(s + vec2(22.0, -8.0), 10.0);
    // Shaft poking out the front.
    gfx::line(s + vec2(34.0, -16.0), s + vec2(54.0, -6.0), 2.4, WOOD);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(k: i64) -> Vec<Decoration> {
        generate_chunk(k, None, world::safe_stop_for_chunk(k))
    }

    /// The chunks that lie wholly inside the region at `from_m..to_m`.
    fn chunks_between(from_m: f32, to_m: f32) -> std::ops::Range<i64> {
        let k = |m: f32| world::chunk_of(world::ROAD_START_Y - m * PX_PER_METER);
        (k(from_m) + 1)..k(to_m)
    }

    fn count(chunks: std::ops::Range<i64>, pick: impl Fn(&Kind) -> bool) -> usize {
        chunks.flat_map(chunk).filter(|d| pick(&d.kind)).count()
    }

    #[test]
    fn nothing_stands_past_the_castles_wall() {
        let castle = world::chunk_of(world::CASTLE_GATE_Y);
        for k in castle - 2..castle + 4 {
            for d in chunk(k) {
                assert!(
                    d.pos.y > world::CASTLE_WALL_Y,
                    "chunk {k}: a prop at {} is behind the wall",
                    d.pos.y
                );
            }
        }
    }

    #[test]
    fn every_region_border_has_one_sign_naming_it() {
        for (i, r) in regions::REGIONS.iter().enumerate().skip(1) {
            let y = world::ROAD_START_Y - r.from_m as f32 * PX_PER_METER;
            let signs: Vec<_> = chunk(world::chunk_of(y))
                .into_iter()
                .filter(|d| matches!(d.kind, Kind::RegionSign { region } if region as usize == i))
                .collect();
            assert_eq!(signs.len(), 1, "{}: {} signs", r.name, signs.len());
            assert!(
                (signs[0].pos.y - y).abs() < 1.0,
                "{}: the sign is off its border",
                r.name
            );
            assert!(
                signs[0].pos.x > world::path_center(y) + PATH_HALF_W,
                "{}: the sign is on the road",
                r.name
            );
        }
    }

    #[test]
    fn the_forest_has_no_signs_wells_or_hay() {
        let forest = chunks_between(0.0, 880.0);
        assert_eq!(
            count(forest, |k| matches!(
                k,
                Kind::RegionSign { .. } | Kind::Well | Kind::Haystack { .. }
            )),
            0
        );
    }

    #[test]
    fn the_village_is_busier_with_houses_and_thinner_with_trees() {
        let forest = chunks_between(100.0, 600.0);
        let village = chunks_between(1100.0, 1400.0);
        let (fk, vk) = (forest.end - forest.start, village.end - village.start);
        let per = |n: usize, k: i64| n as f32 / k as f32;
        let cottages = |k: &Kind| matches!(k, Kind::Cottage { .. });
        let pines = |k: &Kind| matches!(k, Kind::Pine { .. });
        assert!(
            per(count(village.clone(), cottages), vk)
                > 1.5 * per(count(forest.clone(), cottages), fk),
            "the village has no more houses than the forest"
        );
        assert!(
            per(count(village.clone(), pines), vk) < 0.6 * per(count(forest.clone(), pines), fk),
            "the village is as wooded as the forest"
        );
        assert!(
            count(village, |k| matches!(k, Kind::Well)) > 0,
            "no well in the village"
        );
    }

    #[test]
    fn generation_is_deterministic_in_every_region() {
        for m in [300.0, 1000.0, 1250.0, 1450.0] {
            let k = world::chunk_of(world::ROAD_START_Y - m * PX_PER_METER);
            let (a, b) = (chunk(k), chunk(k));
            assert_eq!(a.len(), b.len(), "chunk {k}");
            for (p, q) in a.iter().zip(&b) {
                assert_eq!(p.pos, q.pos, "chunk {k}");
            }
        }
    }
}
