//! The beach world: an endless wooden boardwalk that winds north (up the
//! screen) between the ocean on the left and the dunes on the right.
//!
//! Coordinates are plain pixels in a 3/4 top-down view: +x is right, +y is
//! *down* the screen, so walking "forward" means `y` decreasing without
//! bound. The projection to the screen is just a translation (`View`), and
//! height is a straight upward lift in screen pixels — so the 2.5D read
//! comes from lifted surfaces, cast shadows, and back-to-front sorting of
//! tall props against the player by their ground `y`.
//!
//! Nothing is a fixed map. The path's shape is a pure function of `y`
//! (`path_center`), and scenery is generated chunk by chunk from a seed
//! derived from the chunk index, so it's identical every run and costs
//! nothing until the camera gets near it.

use super::decor::{self, Decoration};
use super::gfx::{self, INK, SHADOW};
use super::monster::MonsterKind;
use macroquad::prelude::*;

/// Logical width of the beach strip. The view centers this horizontally;
/// a wider window just sees more ocean on the left and dunes on the right.
pub const WORLD_W: f32 = 960.0;
/// Half the boardwalk's width.
pub const PATH_HALF_W: f32 = 76.0;
/// How far inside the boardwalk edge the player's feet must stay, so the
/// sprite never pokes through the rope railing.
pub const WALK_MARGIN: f32 = 19.0;
/// Screen-space lift of the boardwalk deck above the sand. Small, but with
/// the cast shadow it's what makes the deck read as raised planks.
pub const BOARDWALK_LIFT: f32 = 6.0;
/// The boardwalk's southern end: the gateway from the spawn hub onto the
/// road (FR15).
pub const BOARDWALK_SOUTH_END: f32 = 150.0;
/// Where the road starts: distance, trail markers and the first bottle are
/// measured from here, not from the spawn point.
pub const ROAD_START_Y: f32 = BOARDWALK_SOUTH_END;
/// The spawn hub: open, free-roam sand south of the gateway, down to here.
/// A safe zone: no lava or monsters are ever placed south of the gateway.
pub const HUB_SOUTH: f32 = BOARDWALK_SOUTH_END + 760.0;
/// Over this stretch south of the gateway the walkable width eases from the
/// deck's to the hub's, so the hub funnels smoothly onto the boardwalk.
const HUB_FUNNEL: f32 = 150.0;
/// Spawn point, in the hub with the gateway in view up the screen.
pub const PLAYER_START_Y: f32 = BOARDWALK_SOUTH_END + 270.0;
/// The south edge of the hub the player can walk to.
pub const SOUTH_LIMIT: f32 = HUB_SOUTH - 30.0;
/// Conversion for the distance readout and the trail markers.
pub const PX_PER_METER: f32 = 24.0;
/// Height of one procedurally generated strip of scenery.
pub const CHUNK_H: f32 = 320.0;

/// Southmost chunk ever generated: enough to fill the screen below the
/// hub when the player walks down to `SOUTH_LIMIT`.
pub(crate) const FIRST_CHUNK: i64 = -4;
const POST_SPACING: f32 = 84.0;
const PLANK: f32 = 14.0;
/// A message-in-a-bottle task trigger appears every this many chunks.
const SPOT_EVERY_N_CHUNKS: i64 = 2;
/// Player-to-bottle distance within which E opens it. Generous enough that
/// the player doesn't have to stand exactly on the bottle, tight enough
/// that the prompt only shows for the bottle they're actually next to.
const SPOT_INTERACT_DIST: f32 = 52.0;
/// Odds that an odd-numbered road chunk has an encounter zone (FR10).
/// Bottles take the even chunks, so the two never crowd each other.
const ZONE_CHANCE: f32 = 0.7;
/// Half the zone's depth along the road: stepping within this of its
/// center starts the encounter.
pub const ZONE_HALF_H: f32 = 26.0;
/// A Tide Titan boss guards the road every this many meters.
pub const BOSS_EVERY_M: u32 = 500;

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// World x of the boardwalk's centerline at height `y`. Two slow sines give
/// a gentle, non-repeating-looking meander; it eases in from dead straight
/// over the first stretch so the opening screen reads as a clear path ahead
/// before it starts to wind. Max slope is ~0.25, so the railing nudges the
/// player sideways along bends rather than shoving them.
pub fn path_center(y: f32) -> f32 {
    let d = -y;
    let ramp = smoothstep(0.0, 700.0, d);
    WORLD_W * 0.5 + ramp * (62.0 * (d * 0.0019).sin() + 26.0 * (d * 0.0047 + 1.1).sin())
}

/// Where dry land meets the water on the left. Tied to the path so the
/// beach between them never pinches shut, with its own wobble so the
/// coast doesn't look like a copy of the path.
pub fn shore_x(y: f32) -> f32 {
    path_center(y) - PATH_HALF_W - 175.0
        + 40.0 * (y * 0.0027 + 0.6).sin()
        + 14.0 * (y * 0.0093).sin()
}

/// Where the flat beach gives way to grassy dunes on the right.
pub fn dune_x(y: f32) -> f32 {
    path_center(y)
        + PATH_HALF_W
        + 105.0
        + 28.0 * (y * 0.0034 + 2.0).sin()
        + 10.0 * (y * 0.011 + 0.4).sin()
}

/// Walkable x range at `y`: the deck's width on the boardwalk, easing out
/// through the funnel to the hub's dry sand south of the gateway.
pub fn walkable_x(y: f32) -> (f32, f32) {
    let c = path_center(y);
    let reach = PATH_HALF_W - WALK_MARGIN;
    if y <= BOARDWALK_SOUTH_END {
        return (c - reach, c + reach);
    }
    let t = smoothstep(BOARDWALK_SOUTH_END, BOARDWALK_SOUTH_END + HUB_FUNNEL, y);
    let left = shore_x(y) + 48.0;
    let right = (dune_x(y) + 90.0).min(WORLD_W - 40.0);
    (
        c - reach + (left - (c - reach)) * t,
        c + reach + (right - (c + reach)) * t,
    )
}

/// Where Professor Pebble, the hub's tutorial NPC (FR9), stands.
pub fn pebble_pos() -> Vec2 {
    vec2(
        path_center(BOARDWALK_SOUTH_END) - 78.0,
        BOARDWALK_SOUTH_END + 215.0,
    )
}

/// How close the player must be to talk to an NPC.
pub const NPC_TALK_DIST: f32 = 56.0;

/// True south of the gateway, in the spawn hub (FR15).
pub fn in_hub(y: f32) -> bool {
    y > BOARDWALK_SOUTH_END
}

/// Keeps a position walkable: on the deck north of the gateway, anywhere
/// on the hub's sand south of it, never past the hub's south edge.
pub fn clamp_to_path(p: Vec2) -> Vec2 {
    let y = p.y.min(SOUTH_LIMIT);
    let (lo, hi) = walkable_x(y);
    vec2(p.x.clamp(lo, hi), y)
}

/// How high the ground is at `y`: the deck's lift on the boardwalk,
/// stepping down to the sand at its south end.
pub fn ground_lift(y: f32) -> f32 {
    BOARDWALK_LIFT * (1.0 - smoothstep(BOARDWALK_SOUTH_END - 8.0, BOARDWALK_SOUTH_END + 8.0, y))
}

pub fn chunk_of(y: f32) -> i64 {
    (-y / CHUNK_H).floor() as i64
}

/// `(north, south)` world-y bounds of chunk `k`; chunk 0 starts at y = 0.
pub fn chunk_span(k: i64) -> (f32, f32) {
    (-(k + 1) as f32 * CHUNK_H, -k as f32 * CHUNK_H)
}

/// The camera. Projection is a pure translation (pixel-rounded so edges
/// stay crisp while it glides).
#[derive(Clone, Copy)]
pub struct View {
    pub offset: Vec2,
}

impl View {
    pub fn new(cam_y: f32) -> Self {
        Self {
            offset: vec2(((screen_width() - WORLD_W) * 0.5).round(), (-cam_y).round()),
        }
    }

    pub fn to_screen(&self, p: Vec2) -> Vec2 {
        p + self.offset
    }

    pub fn top(&self) -> f32 {
        -self.offset.y
    }

    pub fn bottom(&self) -> f32 {
        -self.offset.y + screen_height()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpotState {
    /// Bottle still waiting on the boardwalk.
    Fresh,
    /// Picked up; its task is loading or open.
    Pending,
    Solved,
    Missed,
}

/// A task trigger: a message in a bottle lying on the boardwalk. Each one
/// fires once, and keeps a mark afterward so the trail behind the player
/// shows how they did.
pub struct Spot {
    pub pos: Vec2,
    pub state: SpotState,
}

/// Anything that stands up off the ground and so has to be painted in
/// back-to-front order with the player.
pub enum Tall<'a> {
    Decor(&'a Decoration),
    /// A railing post, with the position of the next post north so it can
    /// draw the rope between them.
    Post {
        base: Vec2,
        next: Vec2,
        bunting: bool,
    },
    Bottle(&'a Spot),
}

impl Tall<'_> {
    pub fn sort_y(&self) -> f32 {
        match self {
            Tall::Decor(d) => d.pos.y,
            Tall::Post { base, .. } => base.y,
            Tall::Bottle(s) => s.pos.y,
        }
    }

    pub fn draw(&self, view: &View, t: f32) {
        match self {
            Tall::Decor(d) => d.draw(view, t),
            Tall::Post {
                base,
                next,
                bunting,
            } => draw_post(view, *base, *next, *bunting),
            Tall::Bottle(s) => draw_bottle(view, s, t),
        }
    }
}

/// A patch of rustling sea-grass across the deck where a wild monster
/// lurks (FR10). Walking into it starts a battle with `kind`.
pub struct EncounterZone {
    pub pos: Vec2,
    pub kind: MonsterKind,
    pub cleared: bool,
}

/// Decides whether chunk `k` gets an encounter zone, where, and which
/// monster. Deterministic per chunk, like the bottles, and never in the
/// spawn hub.
pub fn encounter_for_chunk(k: i64) -> Option<(Vec2, MonsterKind)> {
    if k < 1 || k % 2 == 0 {
        return None;
    }
    // A Tide Titan guards each 500 m milestone: the first odd chunk at or
    // past it gets the boss, always.
    let (north, _) = chunk_span(k);
    let (_, prev_south) = chunk_span(k - 1);
    let milestone_px = BOSS_EVERY_M as f32 * PX_PER_METER;
    let lo_m = ((ROAD_START_Y - prev_south) / milestone_px).ceil().max(1.0) as i64;
    let at = ROAD_START_Y - lo_m as f32 * milestone_px;
    if at <= prev_south && at > north {
        let y = (north + CHUNK_H * 0.5).min(at - 20.0).max(north + 40.0);
        return Some((vec2(path_center(y), y), MonsterKind::TideTitan));
    }
    let h = |n: u64| gfx::hash01((k as u64).wrapping_mul(11).wrapping_add(n) ^ 0xE4C0);
    if h(3) >= ZONE_CHANCE {
        return None;
    }
    let y = north + 80.0 + h(4) * (CHUNK_H - 160.0);
    let meters = ((ROAD_START_Y - y) / PX_PER_METER).max(0.0) as u32;
    Some((vec2(path_center(y), y), MonsterKind::pick(h(5), meters)))
}

pub struct World {
    /// `chunks[i]` holds the scenery for chunk `FIRST_CHUNK + i`.
    chunks: Vec<Vec<Decoration>>,
    pub spots: Vec<Spot>,
    pub zones: Vec<EncounterZone>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        let mut world = Self {
            chunks: Vec::new(),
            spots: Vec::new(),
            zones: Vec::new(),
        };
        world.ensure_generated(-CHUNK_H * 2.0);
        world
    }

    /// Generates every chunk up to (and including) the one containing
    /// `north_y`. Chunks are never dropped: they're small, and bottles need
    /// to remember they've been solved if the player walks back past them.
    pub fn ensure_generated(&mut self, north_y: f32) {
        let want = chunk_of(north_y);
        while FIRST_CHUNK + (self.chunks.len() as i64) <= want {
            let k = FIRST_CHUNK + self.chunks.len() as i64;
            let spot = spot_for_chunk(k);
            self.chunks.push(decor::generate_chunk(k, spot));
            if let Some(pos) = spot {
                self.spots.push(Spot {
                    pos,
                    state: SpotState::Fresh,
                });
            }
            if let Some((pos, kind)) = encounter_for_chunk(k) {
                self.zones.push(EncounterZone {
                    pos,
                    kind,
                    cleared: false,
                });
            }
        }
    }

    /// The closest unopened bottle within reach of `pos`, if any — what
    /// pressing E would open.
    pub fn interactable_spot(&self, pos: Vec2) -> Option<usize> {
        self.spots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.state == SpotState::Fresh)
            .map(|(i, s)| (i, s.pos.distance(pos)))
            .filter(|&(_, d)| d < SPOT_INTERACT_DIST)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// The uncleared encounter zone the player at `pos` has stepped into.
    pub fn encounter_at(&self, pos: Vec2) -> Option<usize> {
        self.zones
            .iter()
            .position(|z| !z.cleared && (z.pos.y - pos.y).abs() < ZONE_HALF_H)
    }

    /// Scenery that could be on screen: tall props stick up to ~110px
    /// above their base and shadows spill a little below, so the band is
    /// padded more on the south side.
    fn visible_decor<'a>(&'a self, view: &View) -> impl Iterator<Item = &'a Decoration> + 'a {
        let north = view.top() - 80.0;
        let south = view.bottom() + 140.0;
        let len = self.chunks.len();
        let lo = ((chunk_of(south) - FIRST_CHUNK).max(0) as usize).min(len);
        let hi = ((chunk_of(north) - FIRST_CHUNK + 1).max(0) as usize).min(len);
        self.chunks[lo..hi.max(lo)]
            .iter()
            .flatten()
            .filter(move |d| d.pos.y > north && d.pos.y < south)
    }

    pub fn tall_items(&self, view: &View) -> Vec<Tall<'_>> {
        let mut items: Vec<Tall<'_>> = self
            .visible_decor(view)
            .filter(|d| !d.is_flat())
            .map(Tall::Decor)
            .collect();

        items.extend(
            self.spots
                .iter()
                .filter(|s| s.state == SpotState::Fresh)
                .filter(|s| s.pos.y > view.top() - 60.0 && s.pos.y < view.bottom() + 60.0)
                .map(Tall::Bottle),
        );

        // Railing posts sit on a fixed spacing measured north from the
        // boardwalk's south end, both sides.
        let first = BOARDWALK_SOUTH_END - 6.0;
        let j_min = ((first - (view.bottom() + 40.0)) / POST_SPACING)
            .ceil()
            .max(0.0) as i64;
        let j_max = ((first - (view.top() - 40.0)) / POST_SPACING).floor() as i64;
        let post_inset = PATH_HALF_W - 2.0;
        for j in j_min..=j_max {
            let y = first - j as f32 * POST_SPACING;
            let ny = y - POST_SPACING;
            for side in [-1.0, 1.0] {
                items.push(Tall::Post {
                    base: vec2(path_center(y) + side * post_inset, y),
                    next: vec2(path_center(ny) + side * post_inset, ny),
                    bunting: j % 3 == 1,
                });
            }
        }

        items
    }

    /// Ocean, surf, wet and dry sand, dunes, then every flat decoration
    /// (shells, towels, ripples...) on top. Painted as world-aligned 4px
    /// rows so the coastline is stable while the camera scrolls instead of
    /// shimmering.
    pub fn draw_ground(&self, view: &View, t: f32) {
        const ROW: f32 = 4.0;
        let deep = Color::new(0.09, 0.45, 0.72, 1.0);
        let mid = Color::new(0.12, 0.60, 0.80, 1.0);
        let shallow = Color::new(0.28, 0.78, 0.84, 1.0);
        let swash = Color::new(0.56, 0.88, 0.84, 1.0);
        let wet_sand = Color::new(0.86, 0.74, 0.52, 1.0);
        let sand = Color::new(0.97, 0.88, 0.66, 1.0);
        let dune = Color::new(0.93, 0.83, 0.57, 1.0);
        let foam = Color::new(1.0, 1.0, 1.0, 0.92);

        let sw = screen_width();
        let ox = view.offset.x;
        let span = |sy: f32, a: f32, b: f32, color: Color| {
            let (a, b) = (a.max(0.0), b.min(sw));
            if b > a {
                draw_rectangle(a, sy, b - a, ROW, color);
            }
        };

        let mut wy = (view.top() / ROW).floor() * ROW;
        while wy < view.bottom() {
            let sy = wy + view.offset.y;
            let row = (wy / ROW) as i64;
            let shore = shore_x(wy) + ox;

            // The wash: a slow in/out surge whose phase travels along the
            // coast, so the foam line ripples instead of moving rigidly.
            let surge = 0.5 + 0.5 * (t * 1.1 + wy * 0.010).sin();
            let surge2 = 0.5 + 0.5 * (t * 0.7 - wy * 0.006 + 2.0).sin();
            let wash = shore - 6.0 + 18.0 * surge * (0.6 + 0.4 * surge2);

            let deep_edge = shore - 170.0 + 18.0 * (wy * 0.013 + t * 0.35).sin();
            let mid_edge = shore - 70.0 + 10.0 * (wy * 0.021 - t * 0.5).sin();
            let shallow_edge = shore - 24.0 + 4.0 * (wy * 0.05 + t * 0.9).sin();
            let wet_end = shore + 30.0;
            let dune_edge = dune_x(wy) + ox;

            span(sy, 0.0, deep_edge, deep);
            span(sy, deep_edge, mid_edge, mid);
            span(sy, mid_edge, shallow_edge, shallow);
            span(sy, shallow_edge, wash, swash);
            span(sy, wash, wet_end - 6.0, wet_sand);
            span(sy, wet_end - 6.0, wet_end, gfx::mix(wet_sand, sand, 0.5));
            span(sy, wet_end, dune_edge - 8.0, sand);
            span(sy, dune_edge - 8.0, dune_edge, gfx::mix(sand, dune, 0.5));
            span(sy, dune_edge, sw, dune);

            // Offshore break line and the foam riding the wash's front.
            span(
                sy,
                mid_edge - 1.5,
                mid_edge + 1.5,
                gfx::with_alpha(WHITE, 0.30),
            );
            span(sy, wash - 2.5, wash + 2.5, foam);
            if gfx::hash01(row as u64) < 0.35 {
                let fx = shallow_edge + (wash - shallow_edge) * gfx::hash01(row as u64 ^ 0xF0A3);
                draw_circle(fx, sy + 2.0, 1.4, gfx::with_alpha(WHITE, 0.7));
            }

            wy += ROW;
        }

        for d in self.visible_decor(view).filter(|d| d.is_flat()) {
            d.draw(view, t);
        }
    }

    /// The raised plank deck plus the bottle pads lying on it.
    pub fn draw_boardwalk(&self, view: &View, t: f32) {
        let north = view.top() - PLANK * 2.0;
        let south = (view.bottom() + PLANK * 2.0).min(BOARDWALK_SOUTH_END);
        if south > north {
            draw_deck(view, north, south);
        }
        for zone in &self.zones {
            if zone.pos.y > view.top() - 60.0 && zone.pos.y < view.bottom() + 60.0 {
                draw_zone(view, zone, t);
            }
        }
        for spot in &self.spots {
            if spot.pos.y > view.top() - 40.0 && spot.pos.y < view.bottom() + 40.0 {
                draw_spot_pad(view, spot, t);
            }
        }
    }

    /// Gulls wheeling overhead, above everything else. Kept in screen
    /// space so there are always a few in view however far the player
    /// walks.
    pub fn draw_overhead(&self, t: f32) {
        let sw = screen_width();

        for i in 0..3 {
            let fi = i as f32;
            let x = (fi * 337.0 + t * (36.0 + fi * 8.0)).rem_euclid(sw + 240.0) - 120.0;
            let y = 70.0 + fi * 130.0 + 18.0 * (t * 0.6 + fi * 2.0).sin();
            let flap = (t * 7.0 + fi * 1.7).sin();
            let scale = 1.0 - fi * 0.12;
            draw_gull(vec2(x + 46.0, y + 84.0), flap, scale, true);
            draw_gull(vec2(x, y), flap, scale, false);
        }
    }
}

/// Decides whether chunk `k` gets a bottle and where. Deterministic per
/// chunk, like the scenery.
fn spot_for_chunk(k: i64) -> Option<Vec2> {
    if k < 0 || k % SPOT_EVERY_N_CHUNKS != 0 {
        return None;
    }
    let (north, _) = chunk_span(k);
    let r1 = gfx::hash01(k as u64 * 7 + 1);
    let r2 = gfx::hash01(k as u64 * 7 + 2);
    let reach = PATH_HALF_W - 30.0;
    // The first bottle is placed on screen at spawn and near the middle of
    // the deck, so the goal is obvious from the first frame.
    let (y, spread) = if k == 0 {
        (-190.0, 0.4)
    } else {
        (north + 60.0 + r1 * (CHUNK_H - 120.0), 1.0)
    };
    Some(vec2(path_center(y) + (r2 * 2.0 - 1.0) * reach * spread, y))
}

/// Tufts of sea-grass growing through the planks, rustling while a monster
/// hides in them; flattened and faded once it's beaten.
fn draw_zone(view: &View, zone: &EncounterZone, t: f32) {
    let lift = vec2(0.0, -BOARDWALK_LIFT);
    let (green, dark) = if zone.cleared {
        (
            Color::new(0.62, 0.66, 0.40, 0.7),
            Color::new(0.45, 0.48, 0.30, 0.7),
        )
    } else {
        (
            Color::new(0.38, 0.72, 0.42, 1.0),
            Color::new(0.20, 0.48, 0.30, 1.0),
        )
    };
    let seed = (zone.pos.y.abs() as u64).wrapping_mul(31);
    for i in 0..26u64 {
        let fx = gfx::hash01(seed + i * 2) * 2.0 - 1.0;
        let fy = gfx::hash01(seed + i * 2 + 1) * 2.0 - 1.0;
        let y = zone.pos.y + fy * ZONE_HALF_H;
        let base = view.to_screen(vec2(path_center(y) + fx * (PATH_HALF_W - 8.0), y)) + lift;
        let h = if zone.cleared {
            4.0
        } else {
            9.0 + 4.0 * gfx::hash01(seed ^ i)
        };
        let sway = if zone.cleared {
            0.0
        } else {
            (t * 3.0 + fx * 4.0 + fy).sin() * 2.5
        };
        for (dx, lean) in [(-2.5f32, -1.5f32), (0.0, 0.0), (2.5, 1.5)] {
            let tip = base + vec2(dx + lean * 1.5 + sway, -h);
            gfx::line(base + vec2(dx, 0.0), tip, 2.4, dark);
            gfx::line(base + vec2(dx, 0.0), tip, 1.3, green);
        }
    }
}

fn draw_deck(view: &View, north: f32, south: f32) {
    let hw = PATH_HALF_W;
    let lift = vec2(0.0, -BOARDWALK_LIFT);
    let gap = Color::new(0.40, 0.26, 0.15, 1.0);
    let beam = Color::new(0.56, 0.38, 0.22, 1.0);
    let k0 = (north / PLANK).floor() as i64;
    let k1 = (south / PLANK).ceil() as i64;

    // Each plank row's four corners in screen space, following the curve
    // at its top and bottom edge so the deck edge stays smooth on bends.
    let rows = (k0..k1).filter_map(|k| {
        let y0 = k as f32 * PLANK;
        let y1 = (y0 + PLANK).min(BOARDWALK_SOUTH_END);
        (y0 < BOARDWALK_SOUTH_END).then(|| (k, y0, y1, path_center(y0), path_center(y1)))
    });
    let rows: Vec<_> = rows.collect();
    let p = |x: f32, y: f32| view.to_screen(vec2(x, y));

    // Shadow cast down-right onto the sand: this offset strip is most of
    // what sells the deck as raised.
    for &(_, y0, y1, c0, c1) in &rows {
        let o = vec2(10.0, 4.0);
        gfx::quad(
            p(c0 - hw, y0) + o,
            p(c0 + hw, y0) + o,
            p(c1 + hw, y1) + o,
            p(c1 - hw, y1) + o,
            Color::new(0.35, 0.20, 0.05, 0.26),
        );
    }

    for &(k, y0, y1, c0, c1) in &rows {
        gfx::quad(
            p(c0 - hw, y0) + lift,
            p(c0 + hw, y0) + lift,
            p(c1 + hw, y1) + lift,
            p(c1 - hw, y1) + lift,
            gap,
        );

        // Sun-bleached driftwood: every plank its own slightly different
        // tone, inset to leave a dark gap line between rows.
        let h = gfx::hash01(k as u64);
        let wood = gfx::mix(
            Color::new(0.80, 0.60, 0.38, 1.0),
            Color::new(0.90, 0.77, 0.58, 1.0),
            h,
        );
        let (a, b) = (y0 + 1.2, y1 - 1.0);
        let (ca, cb) = (path_center(a), path_center(b));
        gfx::quad(
            p(ca - hw, a) + lift,
            p(ca + hw, a) + lift,
            p(cb + hw, b) + lift,
            p(cb - hw, b) + lift,
            wood,
        );
        // Top-edge highlight where the light catches each board.
        gfx::line(
            p(ca - hw, a) + lift + vec2(0.0, 0.6),
            p(ca + hw, a) + lift + vec2(0.0, 0.6),
            1.0,
            gfx::shade(wood, 1.12),
        );

        // Boards are shorter than the deck is wide: a staggered butt joint
        // per row, plus a stroke of grain, breaks up the repetition.
        let mid = (a + b) * 0.5;
        let cm = path_center(mid);
        let seam = cm - hw + 2.0 * hw * (0.25 + 0.5 * gfx::hash01(k as u64 ^ 0x5EA));
        gfx::line(p(seam, a) + lift, p(seam, b) + lift, 1.2, gap);
        let g0 = cm - hw + 2.0 * hw * gfx::hash01(k as u64 ^ 0x6A1);
        let glen = 14.0 + 26.0 * gfx::hash01(k as u64 ^ 0x6A2);
        gfx::line(
            p(g0, mid + 0.5) + lift,
            p((g0 + glen).min(cm + hw - 8.0), mid + 0.5) + lift,
            1.0,
            gfx::shade(wood, 0.86),
        );

        // Nail heads over the stringers.
        for x in [cm - hw + 10.0, cm + hw - 10.0] {
            let nail = p(x, mid) + lift;
            draw_circle(nail.x, nail.y, 1.1, Color::new(0.32, 0.22, 0.14, 0.9));
        }
    }

    // Edge beams along both sides, with an ink line on the outer edge.
    for &(_, y0, y1, c0, c1) in &rows {
        for side in [-1.0f32, 1.0] {
            let outer0 = c0 + side * hw;
            let outer1 = c1 + side * hw;
            let inner0 = outer0 - side * 7.0;
            let inner1 = outer1 - side * 7.0;
            gfx::quad(
                p(outer0, y0) + lift,
                p(inner0, y0) + lift,
                p(inner1, y1) + lift,
                p(outer1, y1) + lift,
                if side < 0.0 {
                    gfx::shade(beam, 1.08)
                } else {
                    beam
                },
            );
            gfx::line(p(outer0, y0) + lift, p(outer1, y1) + lift, 2.0, INK);
        }
    }

    // South end: the deck's front face and a step down to the sand.
    if south >= BOARDWALK_SOUTH_END {
        let y = BOARDWALK_SOUTH_END;
        let c = path_center(y);
        let tl = p(c - hw, y) + lift;
        let face = Color::new(0.50, 0.33, 0.19, 1.0);
        draw_rectangle(tl.x, tl.y, hw * 2.0, BOARDWALK_LIFT, face);
        let step = p(c - hw + 18.0, y);
        draw_rectangle(
            step.x,
            step.y,
            hw * 2.0 - 36.0,
            4.0,
            Color::new(0.80, 0.62, 0.40, 1.0),
        );
        draw_rectangle(step.x, step.y + 4.0, hw * 2.0 - 36.0, 3.0, face);
        draw_rectangle_lines(tl.x, tl.y, hw * 2.0, BOARDWALK_LIFT, 1.5, INK);
    }
}

fn draw_post(view: &View, base: Vec2, next: Vec2, bunting: bool) {
    const POST_H: f32 = 26.0;
    let lift = vec2(0.0, -BOARDWALK_LIFT);
    let s = view.to_screen(base) + lift;
    let top = s + vec2(0.0, -POST_H);
    let rope_start = top + vec2(0.0, 4.0);
    let next_top = view.to_screen(next) + lift + vec2(0.0, -POST_H + 4.0);

    // Rope to the next post north, sagging in the middle. The dark
    // under-stroke keeps it readable over both pale planks and sand.
    let sag = |u: f32| rope_start.lerp(next_top, u) + vec2(0.0, 8.0 * 4.0 * u * (1.0 - u));
    let rope: Vec<Vec2> = (0..=10).map(|i| sag(i as f32 / 10.0)).collect();
    gfx::polyline(&rope, 4.0, Color::new(0.30, 0.20, 0.10, 0.9));
    gfx::polyline(&rope, 2.4, Color::new(0.94, 0.84, 0.62, 1.0));

    if bunting {
        let colors = [
            Color::new(0.98, 0.45, 0.40, 1.0),
            Color::new(1.00, 0.82, 0.28, 1.0),
            Color::new(0.18, 0.72, 0.74, 1.0),
            Color::new(1.00, 0.97, 0.92, 1.0),
        ];
        for i in 1..6 {
            let u = i as f32 / 6.0;
            let a = sag(u - 0.06);
            let b = sag(u + 0.06);
            let tip = (a + b) * 0.5 + vec2(0.0, 10.0);
            draw_triangle(a, b, tip, colors[i % colors.len()]);
            draw_triangle_lines(a, b, tip, 1.0, gfx::with_alpha(INK, 0.5));
        }
    }

    // Darker than the edge beam it's bolted to, so it doesn't vanish
    // into it.
    gfx::ground_shadow(s + vec2(4.0, 1.0), 6.0, 2.2);
    let wood = Color::new(0.44, 0.28, 0.15, 1.0);
    draw_rectangle(top.x - 3.5, top.y, 7.0, POST_H, wood);
    draw_rectangle(top.x - 3.5, top.y, 2.5, POST_H, gfx::shade(wood, 1.4));
    draw_rectangle_lines(top.x - 3.5, top.y, 7.0, POST_H, 1.4, INK);
    gfx::ellipse(top, 4.2, 2.0, Color::new(0.78, 0.60, 0.40, 1.0));
    gfx::ellipse_outline(top, 4.2, 2.0, 1.0, INK);
}

fn draw_spot_pad(view: &View, spot: &Spot, t: f32) {
    let c = view.to_screen(spot.pos) + vec2(0.0, -BOARDWALK_LIFT);
    let gold = Color::new(1.0, 0.80, 0.25, 1.0);

    match spot.state {
        SpotState::Fresh | SpotState::Pending => {
            let pulse = 0.5 + 0.5 * (t * 3.0).sin();
            gfx::ellipse(
                c,
                30.0 + pulse * 3.0,
                15.0 + pulse * 1.5,
                gfx::with_alpha(gold, 0.20),
            );
            // A ring that keeps expanding outward and fading — a beacon
            // you can spot from the far end of the screen.
            let ripple = (t * 0.8).fract();
            gfx::ellipse_outline(
                c,
                14.0 + ripple * 22.0,
                7.0 + ripple * 11.0,
                2.0,
                gfx::with_alpha(gold, 0.9 * (1.0 - ripple)),
            );
            draw_sand_dollar(c, gold);
        }
        SpotState::Solved => {
            let teal = Color::new(0.20, 0.75, 0.68, 1.0);
            gfx::ellipse(c, 16.0, 8.5, teal);
            gfx::ellipse_outline(c, 16.0, 8.5, 1.5, INK);
            gfx::polyline(
                &[
                    c + vec2(-6.0, 0.0),
                    c + vec2(-1.5, 3.5),
                    c + vec2(7.0, -3.5),
                ],
                2.6,
                WHITE,
            );
        }
        SpotState::Missed => {
            gfx::ellipse(c, 14.0, 7.5, Color::new(0.62, 0.50, 0.38, 0.55));
        }
    }
}

fn draw_sand_dollar(c: Vec2, gold: Color) {
    gfx::ellipse(c, 16.0, 8.5, gfx::shade(gold, 0.95));
    gfx::ellipse(c + vec2(-1.5, -1.0), 13.0, 6.5, gfx::shade(gold, 1.12));
    for i in 0..5 {
        let a = i as f32 / 5.0 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let d = vec2(a.cos() * 7.0, a.sin() * 3.6);
        gfx::ellipse(c + d, 2.0, 1.1, gfx::shade(gold, 0.72));
    }
    gfx::ellipse_outline(c, 16.0, 8.5, 1.5, INK);
}

/// The trigger itself: a bobbing, glinting message in a bottle.
fn draw_bottle(view: &View, spot: &Spot, t: f32) {
    let base = view.to_screen(spot.pos) + vec2(0.0, -BOARDWALK_LIFT);
    let bob = (t * 2.4 + spot.pos.y * 0.01).sin();
    let center = base + vec2(0.0, -26.0 - bob * 4.0);
    let tilt = -25.0f32;
    let rad = tilt.to_radians();
    let dir = vec2(rad.cos(), rad.sin());

    gfx::ellipse(base, 12.0 - bob * 1.5, 4.5, SHADOW);

    let glass = Color::new(0.50, 0.85, 0.78, 0.9);
    let neck = center + dir * 16.0;
    gfx::rot_rect(neck, vec2(6.0, 4.4), rad, INK);
    gfx::rot_rect(neck, vec2(5.5, 3.6), rad, glass);
    gfx::ellipse_rot(center, 16.5, 9.5, tilt, glass);
    gfx::ellipse_rot(
        center + vec2(1.0, 1.5),
        13.0,
        6.0,
        tilt,
        gfx::shade(glass, 0.88),
    );
    // The rolled-up message inside, tied with a red ribbon.
    gfx::ellipse_rot(center, 9.5, 4.2, tilt, Color::new(1.0, 0.95, 0.80, 1.0));
    gfx::line(
        center - dir * 6.0,
        center + dir * 6.0,
        1.0,
        Color::new(0.75, 0.62, 0.45, 0.8),
    );
    gfx::rot_rect(
        center,
        vec2(1.2, 4.4),
        rad,
        Color::new(0.85, 0.25, 0.25, 1.0),
    );
    draw_ellipse_lines(center.x, center.y, 16.5, 9.5, tilt, 1.8, INK);
    gfx::rot_rect(
        center + dir * 22.5,
        vec2(3.4, 3.4),
        rad,
        Color::new(0.62, 0.42, 0.24, 1.0),
    );
    gfx::ellipse_rot(
        center + vec2(-6.0, -4.5),
        6.0,
        2.0,
        tilt,
        gfx::with_alpha(WHITE, 0.75),
    );

    let twinkle = (t * 3.1 + spot.pos.x).sin().max(0.0).powi(3);
    gfx::star(
        center + vec2(18.0, -15.0),
        3.0 + 4.0 * twinkle,
        1.0,
        4,
        0.0,
        1.0,
        gfx::with_alpha(WHITE, 0.3 + 0.7 * twinkle),
    );
}

fn draw_gull(c: Vec2, flap: f32, scale: f32, shadow: bool) {
    let span = 17.0 * scale;
    let lift = flap * 6.0 * scale;
    let wing = |side: f32| {
        [
            c,
            c + vec2(side * span * 0.45, -3.0 * scale - lift * 0.6),
            c + vec2(side * span, 2.0 * scale - lift),
        ]
    };
    if shadow {
        let col = Color::new(0.15, 0.20, 0.30, 0.13);
        for side in [-1.0, 1.0] {
            gfx::polyline(&wing(side), 3.0 * scale, col);
        }
        return;
    }
    for side in [-1.0, 1.0] {
        let w = wing(side);
        gfx::polyline(&w, 4.2 * scale, gfx::with_alpha(INK, 0.6));
        gfx::polyline(&w, 2.6 * scale, WHITE);
        gfx::line(
            w[1].lerp(w[2], 0.55),
            w[2],
            2.6 * scale,
            Color::new(0.35, 0.35, 0.40, 1.0),
        );
    }
    gfx::ellipse(c + vec2(0.0, 1.0), 4.0 * scale, 2.6 * scale, WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHUNKS: std::ops::Range<i64> = FIRST_CHUNK..300;

    #[test]
    fn scenery_never_lands_on_the_boardwalk() {
        for k in CHUNKS {
            for d in decor::generate_chunk(k, spot_for_chunk(k)) {
                if d.pos.y >= BOARDWALK_SOUTH_END {
                    continue; // open sand below the start
                }
                let gap = (d.pos.x - path_center(d.pos.y)).abs() - PATH_HALF_W;
                assert!(
                    gap > 10.0,
                    "chunk {k}: prop at {:?} is {gap}px from the deck edge",
                    d.pos
                );
            }
        }
    }

    #[test]
    fn every_bottle_is_reachable_and_on_the_deck() {
        let reach = PATH_HALF_W - WALK_MARGIN;
        for k in CHUNKS {
            if let Some(p) = spot_for_chunk(k) {
                assert!(
                    p.y < ROAD_START_Y,
                    "bottles are on the road, not in the hub"
                );
                assert!(
                    (p.x - path_center(p.y)).abs() <= reach,
                    "chunk {k}: bottle at {p:?}"
                );
            }
        }
    }

    #[test]
    fn generation_is_deterministic() {
        for k in [-1, 0, 7, 123] {
            let a: Vec<Vec2> = decor::generate_chunk(k, spot_for_chunk(k))
                .iter()
                .map(|d| d.pos)
                .collect();
            let b: Vec<Vec2> = decor::generate_chunk(k, spot_for_chunk(k))
                .iter()
                .map(|d| d.pos)
                .collect();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn interact_reaches_only_nearby_fresh_bottles() {
        let mut world = World::new();
        let i = world
            .spots
            .iter()
            .position(|s| s.pos.y < 0.0)
            .expect("first bottle is generated with the world");
        let at = world.spots[i].pos;

        assert_eq!(world.interactable_spot(at + vec2(30.0, 20.0)), Some(i));
        assert_eq!(world.interactable_spot(at + vec2(0.0, 200.0)), None);

        world.spots[i].state = SpotState::Solved;
        assert_eq!(world.interactable_spot(at), None);
    }

    #[test]
    fn clamp_keeps_the_player_on_the_deck() {
        for y in (-20_000..BOARDWALK_SOUTH_END as i32).step_by(37) {
            for x in [-500.0, 0.0, 480.0, 960.0, 2000.0] {
                let p = clamp_to_path(vec2(x, y as f32));
                assert!(p.y <= SOUTH_LIMIT);
                assert!((p.x - path_center(p.y)).abs() <= PATH_HALF_W - WALK_MARGIN + 1e-3);
            }
        }
    }

    #[test]
    fn encounters_are_on_the_road_and_deterministic() {
        let mut any = 0;
        for k in CHUNKS {
            let a = encounter_for_chunk(k);
            assert_eq!(a, encounter_for_chunk(k));
            if let Some((p, _)) = a {
                any += 1;
                assert!(p.y < ROAD_START_Y, "chunk {k}: zone in the hub");
                assert!(
                    spot_for_chunk(k).is_none(),
                    "chunk {k}: zone shares a bottle's chunk"
                );
                assert_eq!(clamp_to_path(p), p, "chunk {k}: zone off the deck");
            }
        }
        assert!(any > 50, "only {any} zones");
    }

    #[test]
    fn a_tide_titan_guards_every_500_m() {
        let bosses: Vec<u32> = CHUNKS
            .filter_map(encounter_for_chunk)
            .filter(|(_, kind)| *kind == MonsterKind::TideTitan)
            .map(|(p, _)| ((ROAD_START_Y - p.y) / PX_PER_METER) as u32)
            .collect();
        // Chunks up to 300 cover ~4000 m: one boss per milestone, each just
        // past its milestone.
        assert!(bosses.len() >= 7, "{bosses:?}");
        for (i, m) in bosses.iter().enumerate() {
            let milestone = (i as u32 + 1) * BOSS_EVERY_M;
            assert!(*m >= milestone && *m < milestone + 30, "{bosses:?}");
        }
    }

    #[test]
    fn tougher_monsters_appear_further_up_the_road() {
        let mut near = Vec::new();
        let mut far = Vec::new();
        for k in CHUNKS {
            if let Some((p, kind)) = encounter_for_chunk(k) {
                let m = ((ROAD_START_Y - p.y) / PX_PER_METER) as u32;
                assert!(m >= kind.info().min_m, "{kind:?} at {m} m");
                if m < 300 {
                    near.push(kind.info().hits as f32);
                } else if m > 1500 {
                    far.push(kind.info().hits as f32);
                }
            }
        }
        let avg = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        assert!(
            avg(&far) > avg(&near),
            "near {} far {}",
            avg(&near),
            avg(&far)
        );
    }

    #[test]
    fn hub_is_wide_dry_and_bounded() {
        for y in (BOARDWALK_SOUTH_END as i32..3000).step_by(23) {
            for x in [-500.0, 0.0, 480.0, 960.0, 2000.0] {
                let p = clamp_to_path(vec2(x, y as f32));
                assert!(p.y <= SOUTH_LIMIT);
                // Never in the water, never off the world.
                assert!(p.x > shore_x(p.y) + 30.0, "wet at {p:?}");
                assert!(p.x < WORLD_W, "off the world at {p:?}");
            }
        }
        // Past the funnel the hub is much wider than the deck.
        let (lo, hi) = walkable_x(PLAYER_START_Y);
        assert!(hi - lo > 3.0 * (PATH_HALF_W - WALK_MARGIN) * 2.0);
        // And the spawn point is walkable.
        let spawn = vec2(path_center(PLAYER_START_Y), PLAYER_START_Y);
        assert_eq!(clamp_to_path(spawn), spawn);
    }

    #[test]
    fn walkable_width_is_continuous_through_the_gateway() {
        let mut prev = walkable_x(BOARDWALK_SOUTH_END - 50.0);
        let mut y = BOARDWALK_SOUTH_END - 50.0;
        while y < BOARDWALK_SOUTH_END + HUB_FUNNEL + 50.0 {
            let w = walkable_x(y);
            assert!(
                (w.0 - prev.0).abs() < 6.0 && (w.1 - prev.1).abs() < 6.0,
                "jump at {y}"
            );
            prev = w;
            y += 2.0;
        }
    }
}
