//! The forest world (FR24): an endless old cobbled road that winds north
//! (up the screen) between a black lake on the left and the woods on the
//! right, with the mad scientist's manor across the water.
//!
//! Coordinates are plain pixels in a 3/4 top-down view: +x is right, +y is
//! *down* the screen, so walking "forward" means `y` decreasing without
//! bound. The projection to the screen is just a translation (`View`), and
//! height is a straight upward lift in screen pixels, so the 2.5D read
//! comes from lifted surfaces, cast shadows, and back-to-front sorting of
//! tall props against the player by their ground `y`.
//!
//! Nothing is a fixed map. The path's shape is a pure function of `y`
//! (`path_center`), and scenery is generated chunk by chunk from a seed
//! derived from the chunk index, so it's identical every run and costs
//! nothing until the camera gets near it.

use super::decor::{self, Decoration};
use super::gfx::{self, INK, LAB_GREEN, LANTERN, MOON, SHADOW};
use super::manor;
use super::monster::{self, MonsterCanvas, MonsterKind, Pose};
use macroquad::prelude::*;

/// Logical width of the world strip. The view centers this horizontally;
/// a wider window just sees more lake on the left and woods on the right.
pub const WORLD_W: f32 = 960.0;
/// Half the road's width.
pub const PATH_HALF_W: f32 = 76.0;
/// How far inside the road's edge the player's feet must stay, so the
/// sprite never pokes through the iron fence.
pub const WALK_MARGIN: f32 = 19.0;
/// Screen-space lift of the raised road above the ground. Small, but with
/// the cast shadow it's what makes the road read as a raised causeway.
pub const ROAD_LIFT: f32 = 6.0;
/// The road's southern end: the gate from the spawn hub onto the road
/// (FR15).
pub const ROAD_SOUTH_END: f32 = 150.0;
/// Where the road starts: distance, milestones and the first flask are
/// measured from here, not from the spawn point.
pub const ROAD_START_Y: f32 = ROAD_SOUTH_END;
/// The spawn hub: open, free-roam ground south of the gate, down to here.
/// A safe zone: no monsters are ever placed south of the gate.
pub const HUB_SOUTH: f32 = ROAD_SOUTH_END + 760.0;
/// Over this stretch south of the gate the walkable width eases from the
/// road's to the hub's, so the hub funnels smoothly onto the road.
const HUB_FUNNEL: f32 = 150.0;
/// Spawn point, in the hub with the gate in view up the screen.
pub const PLAYER_START_Y: f32 = ROAD_SOUTH_END + 270.0;
/// The south edge of the hub the player can walk to.
pub const SOUTH_LIMIT: f32 = HUB_SOUTH - 30.0;
/// Conversion for the distance readout and the milestones.
pub const PX_PER_METER: f32 = 24.0;
/// Height of one procedurally generated strip of scenery.
pub const CHUNK_H: f32 = 320.0;

/// Southmost chunk ever generated: enough to fill the screen below the
/// hub when the player walks down to `SOUTH_LIMIT`.
pub(crate) const FIRST_CHUNK: i64 = -4;
const POST_SPACING: f32 = 84.0;
/// Height of one row of cobbles.
const COBBLE_ROW: f32 = 14.0;
/// A potion-flask task trigger appears every this many chunks.
const SPOT_EVERY_N_CHUNKS: i64 = 2;
/// Player-to-flask distance within which E opens it. Generous enough that
/// the player doesn't have to stand exactly on the flask, tight enough
/// that the prompt only shows for the flask they're actually next to.
const SPOT_INTERACT_DIST: f32 = 52.0;
/// Odds that an odd-numbered road chunk has an encounter zone (FR10).
/// Flasks take the even chunks, so the two never crowd each other.
const ZONE_CHANCE: f32 = 0.7;
/// Half the zone's depth along the road: stepping within this of its
/// center starts the encounter.
pub const ZONE_HALF_H: f32 = 26.0;
/// A Tide Titan boss guards the road every this many meters.
pub const BOSS_EVERY_M: u32 = 500;
/// Every this many chunks (on the odd chunks, from chunk 3) a monster
/// stands in the open on the road and blocks it (FR2), instead of the
/// chance of one hiding in ferns. Six chunks is ~80 m.
const BLOCKER_EVERY_N_CHUNKS: i64 = 6;
/// The player's feet stop this far south of a road-blocking monster.
pub const BLOCK_GAP: f32 = 44.0;
/// How close (along the road) the player must be to a blocking monster to
/// challenge it with E.
pub const BLOCKER_REACH: f32 = BLOCK_GAP + 26.0;
/// Size of a road-blocking monster on the road, relative to its sprite.
const BLOCKER_SCALE: f32 = 1.15;

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// World x of the road's centerline at height `y`. Two slow sines give
/// a gentle, non-repeating-looking meander; it eases in from dead straight
/// over the first stretch so the opening screen reads as a clear path ahead
/// before it starts to wind. Max slope is ~0.25, so the fence nudges the
/// player sideways along bends rather than shoving them.
pub fn path_center(y: f32) -> f32 {
    let d = -y;
    let ramp = smoothstep(0.0, 700.0, d);
    WORLD_W * 0.5 + ramp * (62.0 * (d * 0.0019).sin() + 26.0 * (d * 0.0047 + 1.1).sin())
}

/// Where dry land meets the lake on the left. Tied to the path so the
/// ground between them never pinches shut, with its own wobble so the
/// bank doesn't look like a copy of the path.
pub fn bank_x(y: f32) -> f32 {
    path_center(y) - PATH_HALF_W - 175.0
        + 40.0 * (y * 0.0027 + 0.6).sin()
        + 14.0 * (y * 0.0093).sin()
}

/// Where the open verge gives way to the woods on the right.
pub fn treeline_x(y: f32) -> f32 {
    path_center(y)
        + PATH_HALF_W
        + 105.0
        + 28.0 * (y * 0.0034 + 2.0).sin()
        + 10.0 * (y * 0.011 + 0.4).sin()
}

/// Walkable x range at `y`: the road's width north of the gate, easing out
/// through the funnel to the hub's open ground south of it.
pub fn walkable_x(y: f32) -> (f32, f32) {
    let c = path_center(y);
    let reach = PATH_HALF_W - WALK_MARGIN;
    if y <= ROAD_SOUTH_END {
        return (c - reach, c + reach);
    }
    let t = smoothstep(ROAD_SOUTH_END, ROAD_SOUTH_END + HUB_FUNNEL, y);
    let left = bank_x(y) + 48.0;
    let right = (treeline_x(y) + 90.0).min(WORLD_W - 40.0);
    (
        c - reach + (left - (c - reach)) * t,
        c + reach + (right - (c + reach)) * t,
    )
}

/// Where Professor Pebble, the hub's tutorial NPC (FR9), stands.
pub fn pebble_pos() -> Vec2 {
    vec2(path_center(ROAD_SOUTH_END) - 78.0, ROAD_SOUTH_END + 215.0)
}

/// How close the player must be to talk to an NPC.
pub const NPC_TALK_DIST: f32 = 56.0;

/// True south of the gate, in the spawn hub (FR15).
pub fn in_hub(y: f32) -> bool {
    y > ROAD_SOUTH_END
}

/// Keeps a position walkable: on the road north of the gate, anywhere on
/// the hub's ground south of it, never past the hub's south edge.
pub fn clamp_to_path(p: Vec2) -> Vec2 {
    let y = p.y.min(SOUTH_LIMIT);
    let (lo, hi) = walkable_x(y);
    vec2(p.x.clamp(lo, hi), y)
}

/// How high the ground is at `y`: the road's lift north of the gate,
/// stepping down to the ground at its south end.
pub fn ground_lift(y: f32) -> f32 {
    ROAD_LIFT * (1.0 - smoothstep(ROAD_SOUTH_END - 8.0, ROAD_SOUTH_END + 8.0, y))
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
    /// Flask still waiting on the road.
    Fresh,
    /// Picked up; its task is loading or open.
    Pending,
    Solved,
    Missed,
}

/// A task trigger: a potion flask hovering over a rune stone on the road
/// (FR24). Each one fires once, and its stone keeps a mark afterward so the
/// trail behind the player shows how they did.
pub struct Spot {
    pub pos: Vec2,
    pub state: SpotState,
}

/// Anything that stands up off the ground and so has to be painted in
/// back-to-front order with the player.
pub enum Tall<'a> {
    Decor(&'a Decoration),
    /// A fence post, with the position of the next post north so it can
    /// draw the iron rails between them. `lantern` posts carry a lantern.
    Post {
        base: Vec2,
        next: Vec2,
        lantern: bool,
    },
    Flask(&'a Spot),
    /// A monster standing on the road, blocking it (FR2).
    Blocker(&'a EncounterZone),
}

impl Tall<'_> {
    pub fn sort_y(&self) -> f32 {
        match self {
            Tall::Decor(d) => d.pos.y,
            Tall::Post { base, .. } => base.y,
            Tall::Flask(s) => s.pos.y,
            Tall::Blocker(z) => z.pos.y,
        }
    }

    pub fn draw(&self, view: &View, t: f32) {
        match self {
            Tall::Decor(d) => d.draw(view, t),
            Tall::Post {
                base,
                next,
                lantern,
            } => draw_post(view, *base, *next, *lantern, t),
            Tall::Flask(s) => draw_flask(view, s, t),
            Tall::Blocker(z) => draw_blocker(view, z, t),
        }
    }
}

/// Where a monster waits on the road (FR10, FR2). Most hide in a patch of
/// ferns and brambles across the road, and walking into it starts a
/// battle with `kind`. A road blocker (`blocks`) stands in the open
/// instead: the player can't get past it, and challenges it with E.
pub struct EncounterZone {
    pub pos: Vec2,
    pub kind: MonsterKind,
    /// Stands on the road and blocks it (FR2), rather than lurking.
    pub blocks: bool,
    pub cleared: bool,
    /// Off-screen canvas for drawing a blocker on the road.
    canvas: MonsterCanvas,
}

/// What `encounter_for_chunk` places: where, which monster, and whether
/// it blocks the road (FR2) or lurks in ferns (FR10).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Encounter {
    pub pos: Vec2,
    pub kind: MonsterKind,
    pub blocks: bool,
}

/// Decides whether chunk `k` gets an encounter, where, which monster, and
/// whether it blocks the road. Deterministic per chunk, like the flasks,
/// and never in the spawn hub.
pub fn encounter_for_chunk(k: i64) -> Option<Encounter> {
    if k < 1 || k % 2 == 0 {
        return None;
    }
    // A Tide Titan guards each 500 m milestone: the first odd chunk at or
    // past it gets the boss, always, standing across the road.
    let (north, _) = chunk_span(k);
    let (_, prev_south) = chunk_span(k - 1);
    let milestone_px = BOSS_EVERY_M as f32 * PX_PER_METER;
    let lo_m = ((ROAD_START_Y - prev_south) / milestone_px).ceil().max(1.0) as i64;
    let at = ROAD_START_Y - lo_m as f32 * milestone_px;
    if at <= prev_south && at > north {
        let y = (north + CHUNK_H * 0.5).min(at - 20.0).max(north + 40.0);
        return Some(Encounter {
            pos: vec2(path_center(y), y),
            kind: MonsterKind::TideTitan,
            blocks: true,
        });
    }
    let h = |n: u64| gfx::hash01((k as u64).wrapping_mul(11).wrapping_add(n) ^ 0xE4C0);
    // Every sixth chunk from chunk 3 always has a monster, out on the road.
    let blocks = k % BLOCKER_EVERY_N_CHUNKS == 3;
    if !blocks && h(3) >= ZONE_CHANCE {
        return None;
    }
    let y = north + 80.0 + h(4) * (CHUNK_H - 160.0);
    let meters = ((ROAD_START_Y - y) / PX_PER_METER).max(0.0) as u32;
    Some(Encounter {
        pos: vec2(path_center(y), y),
        kind: MonsterKind::pick(h(5), meters),
        blocks,
    })
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
    /// `north_y`. Chunks are never dropped: they're small, and flasks need
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
            if let Some(e) = encounter_for_chunk(k) {
                self.zones.push(EncounterZone {
                    pos: e.pos,
                    kind: e.kind,
                    blocks: e.blocks,
                    cleared: false,
                    canvas: MonsterCanvas::default(),
                });
            }
        }
    }

    /// The closest unopened flask within reach of `pos`, if any: what
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

    /// The uncleared fern patch the player at `pos` has stepped into.
    /// Road blockers don't ambush: they're challenged with E instead.
    pub fn encounter_at(&self, pos: Vec2) -> Option<usize> {
        self.zones
            .iter()
            .position(|z| !z.cleared && !z.blocks && (z.pos.y - pos.y).abs() < ZONE_HALF_H)
    }

    /// The undefeated road blocker (FR2) the player at `pos` is close
    /// enough to challenge: what pressing E would fight.
    pub fn blocker_in_reach(&self, pos: Vec2) -> Option<usize> {
        self.zones.iter().position(|z| {
            z.blocks && !z.cleared && pos.y > z.pos.y && pos.y - z.pos.y < BLOCKER_REACH
        })
    }

    /// Keeps `p` south of every undefeated road blocker (FR2): the road is
    /// shut until the monster is beaten. Only acts within a band just past
    /// the blocking line, far more than a frame's movement or a knockback,
    /// so a player who is somehow north of a blocker isn't yanked back.
    pub fn clamp_blocked(&self, p: Vec2) -> Vec2 {
        let mut p = p;
        for z in self.zones.iter().filter(|z| z.blocks && !z.cleared) {
            let line = z.pos.y + BLOCK_GAP;
            if p.y < line && p.y > line - 60.0 {
                p.y = line;
            }
        }
        p
    }

    /// Scenery that could be on screen: tall props stick up to ~130px
    /// above their base and shadows spill a little below, so the band is
    /// padded more on the south side.
    fn visible_decor<'a>(&'a self, view: &View) -> impl Iterator<Item = &'a Decoration> + 'a {
        let north = view.top() - 80.0;
        let south = view.bottom() + 160.0;
        let len = self.chunks.len();
        let lo = ((chunk_of(south) - FIRST_CHUNK).max(0) as usize).min(len);
        let hi = ((chunk_of(north) - FIRST_CHUNK + 1).max(0) as usize).min(len);
        self.chunks[lo..hi.max(lo)]
            .iter()
            .flatten()
            .filter(move |d| d.pos.y > north && d.pos.y < south)
    }

    /// Fence posts on screen: a fixed spacing measured north from the
    /// road's south end, both sides. Every third pair carries a lantern.
    fn posts(view: &View) -> impl Iterator<Item = (Vec2, Vec2, bool)> {
        let first = ROAD_SOUTH_END - 6.0;
        let j_min = ((first - (view.bottom() + 40.0)) / POST_SPACING)
            .ceil()
            .max(0.0) as i64;
        let j_max = ((first - (view.top() - 40.0)) / POST_SPACING).floor() as i64;
        let post_inset = PATH_HALF_W - 2.0;
        (j_min..=j_max).flat_map(move |j| {
            let y = first - j as f32 * POST_SPACING;
            let ny = y - POST_SPACING;
            [-1.0f32, 1.0].map(|side| {
                (
                    vec2(path_center(y) + side * post_inset, y),
                    vec2(path_center(ny) + side * post_inset, ny),
                    j % 3 == 1,
                )
            })
        })
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
                .map(Tall::Flask),
        );
        items.extend(
            self.zones
                .iter()
                .filter(|z| z.blocks && !z.cleared)
                .filter(|z| z.pos.y > view.top() - 40.0 && z.pos.y < view.bottom() + 160.0)
                .map(Tall::Blocker),
        );
        items.extend(Self::posts(view).map(|(base, next, lantern)| Tall::Post {
            base,
            next,
            lantern,
        }));
        items
    }

    /// Lake, muddy bank, forest floor and the woods' darker ground, then
    /// every flat decoration (leaves, lily pads, wisps...) on top. Painted
    /// as world-aligned 4px rows so the bank is stable while the camera
    /// scrolls instead of shimmering.
    pub fn draw_ground(&self, view: &View, t: f32) {
        const ROW: f32 = 4.0;
        let deep = Color::new(0.03, 0.05, 0.09, 1.0);
        let mid = Color::new(0.05, 0.08, 0.13, 1.0);
        let shallow = Color::new(0.08, 0.12, 0.15, 1.0);
        let mud = Color::new(0.17, 0.13, 0.11, 1.0);
        let floor = Color::new(0.12, 0.17, 0.12, 1.0);
        let woods = Color::new(0.07, 0.11, 0.08, 1.0);

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
            let bank = bank_x(wy) + ox;
            // A slow, small lapping at the water's edge.
            let lap = bank - 4.0 + 3.0 * (t * 0.6 + wy * 0.012).sin();
            let deep_edge = bank - 170.0 + 18.0 * (wy * 0.013 + t * 0.15).sin();
            let mid_edge = bank - 70.0 + 10.0 * (wy * 0.021 - t * 0.2).sin();
            let trees = treeline_x(wy) + ox;
            // The floor gets slightly patchy, row by row, like moss.
            let patch = gfx::hash01((row as u64 / 3) ^ 0x5A11) * 0.06;
            let floor_row = gfx::shade(floor, 0.97 + patch);

            span(sy, 0.0, deep_edge, deep);
            span(sy, deep_edge, mid_edge, mid);
            span(sy, mid_edge, lap, shallow);
            span(sy, lap, bank + 28.0, mud);
            span(sy, bank + 28.0, bank + 34.0, gfx::mix(mud, floor_row, 0.5));
            span(sy, bank + 34.0, trees - 10.0, floor_row);
            span(sy, trees - 10.0, trees, gfx::mix(floor_row, woods, 0.5));
            span(sy, trees, sw, woods);

            // Moonlight on the water's edge, and the odd shimmer on the
            // lake drifting slowly.
            span(sy, lap - 1.5, lap + 0.5, gfx::with_alpha(MOON, 0.22));
            if gfx::hash01(row as u64) < 0.10 {
                let u = gfx::hash01(row as u64 ^ 0xF0A3);
                let x = deep_edge + (lap - deep_edge) * u + 8.0 * (t * 0.3 + u * 9.0).sin();
                span(sy, x, x + 10.0 + 14.0 * u, gfx::with_alpha(MOON, 0.06));
            }

            wy += ROW;
        }

        for d in self.visible_decor(view).filter(|d| d.is_flat()) {
            d.draw(view, t);
        }
    }

    /// The manor across the lake (FR24): pinned to the screen over the
    /// water rather than placed in the world, so it's always there ahead
    /// of the player, drifting slowly as the road winds and the camera
    /// climbs. Drawn after the ground and before the road and tall props.
    pub fn draw_manor(&self, view: &View, t: f32, reduce_flashing: bool) {
        let h = screen_height();
        let cam = view.top();
        // Where the lake sits on screen, from the path's slow meander only
        // (not the bank's wobble), so the manor slides gently.
        let lake_right = path_center(cam + h * 0.4) - PATH_HALF_W - 175.0 + view.offset.x;
        let scale = (h / 600.0 * 0.72).clamp(0.6, 1.0);
        let x = (lake_right - 140.0 * scale).max(104.0 * scale);
        let y = h * 0.42 + 22.0 * (cam * 0.0009).sin();
        manor::draw(vec2(x, y), scale, t, true, reduce_flashing);
    }

    /// The raised cobbled road plus what lies on it: the fern patches, the
    /// wards in front of road blockers, and the rune stones under the flasks.
    pub fn draw_road(&self, view: &View, t: f32) {
        let north = view.top() - COBBLE_ROW * 2.0;
        let south = (view.bottom() + COBBLE_ROW * 2.0).min(ROAD_SOUTH_END);
        if south > north {
            draw_cobbles(view, north, south);
        }
        for zone in &self.zones {
            if zone.pos.y > view.top() - 60.0 && zone.pos.y < view.bottom() + 60.0 {
                if zone.blocks {
                    draw_ward(view, zone, t);
                } else {
                    draw_zone(view, zone, t);
                }
            }
        }
        for spot in &self.spots {
            if spot.pos.y > view.top() - 40.0 && spot.pos.y < view.bottom() + 40.0 {
                draw_spot_pad(view, spot, t);
            }
        }
    }

    /// Pools of light on the ground (FR24): lanterns on the fence, the
    /// props that glow, and the flasks waiting on the road. Painted after
    /// the road and before the depth-sorted pass, so they light the ground
    /// under the things that cast them.
    pub fn draw_lights(&self, view: &View, t: f32) {
        let mut lights = Vec::new();
        for d in self.visible_decor(view) {
            d.lights(t, &mut lights);
        }
        for (base, _, lantern) in Self::posts(view) {
            if lantern {
                let flick = 0.9 + 0.1 * (t * 9.0 + base.y).sin() * (t * 5.3 + base.x).sin();
                lights.push((base, 64.0 * flick, gfx::with_alpha(LANTERN, 0.22)));
            }
        }
        for s in self.spots.iter().filter(|s| s.state == SpotState::Fresh) {
            let pulse = 0.85 + 0.15 * (t * 3.0).sin();
            lights.push((s.pos, 58.0 * pulse, gfx::with_alpha(flask_color(s), 0.22)));
        }
        for (p, r, color) in lights {
            let c = view.to_screen(p) - vec2(0.0, ground_lift(p.y));
            if c.y > -r && c.y < screen_height() + r {
                gfx::glow(c, r, r * 0.55, color);
            }
        }
    }

    /// Low mist drifting across the road and the lake. Very faint, so it
    /// sets the mood without hiding anything.
    pub fn draw_mist(&self, view: &View, t: f32) {
        const BAND: f32 = 230.0;
        let j0 = (view.top() / BAND).floor() as i64 - 1;
        let j1 = (view.bottom() / BAND).ceil() as i64 + 1;
        for j in j0..=j1 {
            for n in 0..2u64 {
                let seed = (j as u64).wrapping_mul(7).wrapping_add(n) ^ 0x0015_7000;
                let span = WORLD_W + 600.0;
                let x = (gfx::hash01(seed) * span + t * (8.0 + 6.0 * gfx::hash01(seed ^ 1)))
                    .rem_euclid(span)
                    - 300.0;
                let y = j as f32 * BAND + gfx::hash01(seed ^ 2) * BAND;
                let c = view.to_screen(vec2(x, y));
                gfx::glow(c, 200.0, 36.0, gfx::with_alpha(MOON, 0.045));
            }
        }
    }

    /// Bats flitting across the screen and fireflies over the ground,
    /// above everything else. The bats are in screen space so there are
    /// always a few in view; the fireflies belong to the world and drift
    /// with it. Under `reduce_flashing` the fireflies glow steadily.
    pub fn draw_overhead(&self, view: &View, t: f32, reduce_flashing: bool) {
        let sw = screen_width();
        for i in 0..3 {
            let fi = i as f32;
            let x = (fi * 337.0 + t * (48.0 + fi * 10.0)).rem_euclid(sw + 240.0) - 120.0;
            let y = 80.0 + fi * 140.0 + 22.0 * (t * 0.9 + fi * 2.0).sin();
            let flap = (t * 11.0 + fi * 1.7).sin();
            manor::bat(vec2(x, y), 1.4 - fi * 0.15, flap);
        }

        const BAND: f32 = 160.0;
        let j0 = (view.top() / BAND).floor() as i64;
        let j1 = (view.bottom() / BAND).ceil() as i64;
        for j in j0..=j1 {
            for n in 0..3u64 {
                let seed = (j as u64).wrapping_mul(13).wrapping_add(n) ^ 0xF1_F1;
                let y = j as f32 * BAND + gfx::hash01(seed) * BAND;
                let c = path_center(y);
                // Off the road: over the lakeside or the verge.
                let side = if gfx::hash01(seed ^ 3) < 0.5 {
                    -1.0
                } else {
                    1.0
                };
                let x = c + side * (PATH_HALF_W + 30.0 + gfx::hash01(seed ^ 4) * 140.0);
                let ph = gfx::hash01(seed ^ 5) * std::f32::consts::TAU;
                let p = view.to_screen(vec2(
                    x + 18.0 * (t * 0.5 + ph).sin(),
                    y - 20.0 + 10.0 * (t * 0.7 + ph * 1.3).sin(),
                ));
                let pulse = if reduce_flashing {
                    0.7
                } else {
                    (0.5 + 0.5 * (t * 1.4 + ph).sin()).powi(2)
                };
                let col = Color::new(0.85, 1.0, 0.45, 1.0);
                gfx::glow(p, 12.0, 12.0, gfx::with_alpha(col, 0.35 * pulse));
                draw_circle(p.x, p.y, 1.6, gfx::with_alpha(col, 0.4 + 0.6 * pulse));
            }
        }
    }
}

/// Decides whether chunk `k` gets a flask and where. Deterministic per
/// chunk, like the scenery.
fn spot_for_chunk(k: i64) -> Option<Vec2> {
    if k < 0 || k % SPOT_EVERY_N_CHUNKS != 0 {
        return None;
    }
    let (north, _) = chunk_span(k);
    let r1 = gfx::hash01(k as u64 * 7 + 1);
    let r2 = gfx::hash01(k as u64 * 7 + 2);
    let reach = PATH_HALF_W - 30.0;
    // The first flask is placed on screen at spawn and near the middle of
    // the road, so the goal is obvious from the first frame.
    let (y, spread) = if k == 0 {
        (-190.0, 0.4)
    } else {
        (north + 60.0 + r1 * (CHUNK_H - 120.0), 1.0)
    };
    Some(vec2(path_center(y) + (r2 * 2.0 - 1.0) * reach * spread, y))
}

/// Each flask's potion colour: green, violet or pink, chosen by where it
/// sits so it's stable.
fn flask_color(spot: &Spot) -> Color {
    const POTIONS: [Color; 3] = [
        LAB_GREEN,
        Color::new(0.74, 0.50, 1.0, 1.0),
        Color::new(1.0, 0.50, 0.78, 1.0),
    ];
    POTIONS[(gfx::hash01(spot.pos.y.abs() as u64) * 3.0) as usize % 3]
}

/// Ferns and brambles growing through the cobbles, rustling while a
/// monster hides in them (with a pair of eyes peeking out now and then);
/// flattened and faded once it's beaten.
fn draw_zone(view: &View, zone: &EncounterZone, t: f32) {
    let lift = vec2(0.0, -ROAD_LIFT);
    let (green, dark) = if zone.cleared {
        (
            Color::new(0.34, 0.32, 0.26, 0.7),
            Color::new(0.22, 0.20, 0.17, 0.7),
        )
    } else {
        (
            Color::new(0.22, 0.52, 0.34, 1.0),
            Color::new(0.08, 0.26, 0.18, 1.0),
        )
    };
    let seed = (zone.pos.y.abs() as u64).wrapping_mul(31);
    for i in 0..20u64 {
        let fx = gfx::hash01(seed + i * 2) * 2.0 - 1.0;
        let fy = gfx::hash01(seed + i * 2 + 1) * 2.0 - 1.0;
        let y = zone.pos.y + fy * ZONE_HALF_H;
        let base = view.to_screen(vec2(path_center(y) + fx * (PATH_HALF_W - 10.0), y)) + lift;
        let h = if zone.cleared {
            5.0
        } else {
            13.0 + 5.0 * gfx::hash01(seed ^ i)
        };
        let sway = if zone.cleared {
            0.0
        } else {
            (t * 3.0 + fx * 4.0 + fy).sin() * 2.5
        };
        // A fern: fronds arching out from the middle.
        for k in [-2.0f32, -1.0, 0.0, 1.0, 2.0] {
            let a = -std::f32::consts::FRAC_PI_2 + k * 0.45;
            let tip = base + vec2(a.cos() * h * 0.9 + sway, a.sin() * h + k.abs() * h * 0.25);
            let mid = base.lerp(tip, 0.5) + vec2(0.0, -h * 0.2);
            gfx::ribbon(&[base, mid, tip], &[2.0, 1.8, 0.3], dark);
            gfx::ribbon(&[base, mid, tip], &[1.0, 1.0, 0.2], green);
        }
    }
    if !zone.cleared {
        // Bramble loops across the patch.
        let thorn = Color::new(0.30, 0.16, 0.24, 1.0);
        for i in 0..3u64 {
            let y = zone.pos.y + (gfx::hash01(seed ^ (i + 40)) * 2.0 - 1.0) * ZONE_HALF_H * 0.8;
            let c = view.to_screen(vec2(path_center(y), y)) + lift;
            let pts: Vec<Vec2> = (0..=10)
                .map(|k| {
                    let u = k as f32 / 10.0;
                    c + vec2(
                        (u - 0.5) * (PATH_HALF_W * 1.6),
                        -6.0 * (u * 9.0 + i as f32).sin() - 3.0,
                    )
                })
                .collect();
            gfx::polyline(&pts, 1.6, thorn);
        }
        // Eyes peeking out of the middle, blinking now and then.
        let eyes = view.to_screen(vec2(zone.pos.x + 14.0, zone.pos.y)) + lift + vec2(0.0, -8.0);
        let open = (t * 0.6 + zone.pos.y * 0.01).fract() > 0.08;
        for dx in [-3.5f32, 3.5] {
            if open {
                draw_circle(eyes.x + dx, eyes.y, 2.0, LAB_GREEN);
                draw_circle(eyes.x + dx, eyes.y, 0.9, INK);
            } else {
                gfx::line(
                    eyes + vec2(dx - 2.0, 0.0),
                    eyes + vec2(dx + 2.0, 0.0),
                    1.2,
                    LAB_GREEN,
                );
            }
        }
    }
}

/// The ward a road-blocking monster (FR2) holds across the road: a line of
/// green lab runes from fence to fence just in front of it, which is what
/// stops the player. Gone, with the monster, once it's beaten.
fn draw_ward(view: &View, zone: &EncounterZone, t: f32) {
    if zone.cleared {
        return;
    }
    let lift = vec2(0.0, -ROAD_LIFT);
    let y = zone.pos.y + BLOCK_GAP - 8.0;
    let c = path_center(y);
    let hw = PATH_HALF_W - 8.0;
    let pulse = 0.6 + 0.4 * (t * 2.6 + zone.pos.y * 0.01).sin();
    let a = view.to_screen(vec2(c - hw, y)) + lift;
    let b = view.to_screen(vec2(c + hw, y)) + lift;
    gfx::glow(
        a.lerp(b, 0.5),
        hw * 1.3,
        16.0,
        gfx::with_alpha(LAB_GREEN, 0.20 * pulse),
    );
    gfx::line(a, b, 3.0, gfx::with_alpha(INK, 0.5));
    gfx::line(a, b, 1.6, gfx::with_alpha(LAB_GREEN, 0.5 + 0.4 * pulse));
    // Rune marks along it, drifting slowly.
    const N: usize = 7;
    for i in 0..N {
        let u = (i as f32 + 0.5 + 0.15 * (t * 0.7 + i as f32).sin()) / N as f32;
        let p = a.lerp(b, u);
        let r = 4.0;
        let col = gfx::with_alpha(LAB_GREEN, 0.55 + 0.45 * pulse);
        gfx::ellipse_outline(p, r, r * 0.55, 1.3, col);
        gfx::line(p + vec2(0.0, -r * 0.9), p + vec2(0.0, -r * 2.2), 1.2, col);
    }
}

/// A road-blocking monster (FR2) standing on the road, idling, with its
/// shadow on the cobbles.
fn draw_blocker(view: &View, zone: &EncounterZone, t: f32) {
    let ground = view.to_screen(zone.pos) + vec2(0.0, -ROAD_LIFT);
    let scale = if zone.kind.info().boss {
        BLOCKER_SCALE * 1.3
    } else {
        BLOCKER_SCALE
    };
    gfx::ellipse(ground + vec2(4.0, 2.0), 30.0 * scale, 8.0 * scale, SHADOW);
    monster::draw(
        &zone.canvas,
        &monster::Species::Builtin(zone.kind),
        ground,
        scale,
        Pose::Idle,
        Vec2::ZERO,
        0,
        t,
        false,
    );
}

/// The raised road: rows of rounded cobbles over dark mortar, with mossy
/// stone curbs along both sides and a step down at the south end.
fn draw_cobbles(view: &View, north: f32, south: f32) {
    let hw = PATH_HALF_W;
    let lift = vec2(0.0, -ROAD_LIFT);
    let mortar = Color::new(0.16, 0.16, 0.20, 1.0);
    let curb = Color::new(0.30, 0.31, 0.36, 1.0);
    let k0 = (north / COBBLE_ROW).floor() as i64;
    let k1 = (south / COBBLE_ROW).ceil() as i64;

    // Each row's four corners in screen space, following the curve at its
    // top and bottom edge so the road's edge stays smooth on bends.
    let rows: Vec<_> = (k0..k1)
        .filter_map(|k| {
            let y0 = k as f32 * COBBLE_ROW;
            let y1 = (y0 + COBBLE_ROW).min(ROAD_SOUTH_END);
            (y0 < ROAD_SOUTH_END).then(|| (k, y0, y1, path_center(y0), path_center(y1)))
        })
        .collect();
    let p = |x: f32, y: f32| view.to_screen(vec2(x, y));

    // Shadow cast down-right onto the ground: this offset strip is most of
    // what sells the road as raised.
    for &(_, y0, y1, c0, c1) in &rows {
        let o = vec2(10.0, 4.0);
        gfx::quad(
            p(c0 - hw, y0) + o,
            p(c0 + hw, y0) + o,
            p(c1 + hw, y1) + o,
            p(c1 - hw, y1) + o,
            gfx::with_alpha(SHADOW, 0.45),
        );
    }

    const STONE_W: f32 = 19.0;
    for &(k, y0, y1, c0, c1) in &rows {
        gfx::quad(
            p(c0 - hw, y0) + lift,
            p(c0 + hw, y0) + lift,
            p(c1 + hw, y1) + lift,
            p(c1 - hw, y1) + lift,
            mortar,
        );
        // Cobbles: staggered every other row, each its own worn shade.
        let stagger = if k % 2 == 0 { 0.0 } else { STONE_W * 0.5 };
        let (a, b) = (y0 + 1.4, y1 - 1.0);
        let (ca, cb) = (path_center(a), path_center(b));
        let inner = hw - 7.0;
        let mut i = -1;
        loop {
            let u0 = -inner + stagger + i as f32 * STONE_W + 1.2;
            let u1 = u0 + STONE_W - 2.4;
            i += 1;
            if u0 >= inner {
                break;
            }
            let (u0, u1) = (u0.max(-inner + 1.0), u1.min(inner - 1.0));
            if u1 - u0 < 3.0 {
                continue;
            }
            let h = gfx::hash01((k as u64).wrapping_mul(17).wrapping_add(i as u64));
            let stone = gfx::mix(
                Color::new(0.29, 0.29, 0.35, 1.0),
                Color::new(0.43, 0.42, 0.48, 1.0),
                h,
            );
            gfx::quad(
                p(ca + u0, a) + lift,
                p(ca + u1, a) + lift,
                p(cb + u1, b) + lift,
                p(cb + u0, b) + lift,
                stone,
            );
            // Moonlight along each stone's top edge.
            gfx::line(
                p(ca + u0 + 1.0, a) + lift + vec2(0.0, 0.6),
                p(ca + u1 - 1.0, a) + lift + vec2(0.0, 0.6),
                1.0,
                gfx::with_alpha(MOON, 0.30),
            );
            // Now and then a tuft of moss in the joint beside it.
            if h > 0.93 {
                let m = p(cb + u1 + 1.2, b) + lift;
                gfx::ellipse(m, 2.6, 1.4, Color::new(0.24, 0.40, 0.22, 1.0));
            }
        }
    }

    // Stone curbs along both sides, with an ink line on the outer edge.
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
                    gfx::mix(curb, MOON, 0.12)
                } else {
                    curb
                },
            );
            gfx::line(p(outer0, y0) + lift, p(outer1, y1) + lift, 2.0, INK);
        }
    }

    // South end: the road's front face and a stone step down.
    if south >= ROAD_SOUTH_END {
        let y = ROAD_SOUTH_END;
        let c = path_center(y);
        let tl = p(c - hw, y) + lift;
        let face = Color::new(0.24, 0.24, 0.29, 1.0);
        draw_rectangle(tl.x, tl.y, hw * 2.0, ROAD_LIFT, face);
        let step = p(c - hw + 18.0, y);
        draw_rectangle(
            step.x,
            step.y,
            hw * 2.0 - 36.0,
            4.0,
            Color::new(0.42, 0.42, 0.48, 1.0),
        );
        draw_rectangle(step.x, step.y + 4.0, hw * 2.0 - 36.0, 3.0, face);
        draw_rectangle_lines(tl.x, tl.y, hw * 2.0, ROAD_LIFT, 1.5, INK);
    }
}

/// An iron fence post with a spear tip, and two iron rails to the next
/// post north. `lantern` posts carry a hanging lantern.
fn draw_post(view: &View, base: Vec2, next: Vec2, lantern: bool, t: f32) {
    const POST_H: f32 = 28.0;
    let iron = Color::new(0.11, 0.11, 0.15, 1.0);
    let lift = vec2(0.0, -ROAD_LIFT);
    let s = view.to_screen(base) + lift;
    let top = s + vec2(0.0, -POST_H);
    let next_s = view.to_screen(next) + lift;

    for h in [POST_H - 5.0, 9.0] {
        let a = s + vec2(0.0, -h);
        let b = next_s + vec2(0.0, -h);
        gfx::line(a, b, 3.0, gfx::with_alpha(INK, 0.6));
        gfx::line(a, b, 1.8, iron);
    }

    gfx::ground_shadow(s + vec2(4.0, 1.0), 5.0, 2.0);
    draw_rectangle(top.x - 2.2, top.y, 4.4, POST_H, iron);
    draw_rectangle(top.x - 2.2, top.y, 1.2, POST_H, gfx::with_alpha(MOON, 0.35));
    draw_triangle(
        top + vec2(-3.6, 0.5),
        top + vec2(3.6, 0.5),
        top + vec2(0.0, -7.0),
        iron,
    );
    draw_rectangle(top.x - 3.4, top.y + 2.0, 6.8, 2.0, iron);

    if lantern {
        let swing = 1.2 * (t * 1.8 + base.y * 0.1).sin();
        let lamp = top + vec2(swing, -16.0);
        gfx::line(top + vec2(0.0, -6.0), lamp + vec2(0.0, -4.0), 1.0, iron);
        let flick = 0.85 + 0.15 * (t * 9.0 + base.y).sin() * (t * 5.3 + base.x).sin();
        gfx::glow(lamp, 18.0, 18.0, gfx::with_alpha(LANTERN, 0.35 * flick));
        draw_rectangle(
            lamp.x - 3.5,
            lamp.y - 4.0,
            7.0,
            8.0,
            gfx::shade(LANTERN, flick),
        );
        draw_rectangle_lines(lamp.x - 3.5, lamp.y - 4.0, 7.0, 8.0, 1.2, iron);
        draw_triangle(
            lamp + vec2(-5.0, -4.0),
            lamp + vec2(5.0, -4.0),
            lamp + vec2(0.0, -8.0),
            iron,
        );
    }
}

/// The rune stone set into the road under each flask: glowing while its
/// flask waits, marked once it's been answered.
fn draw_spot_pad(view: &View, spot: &Spot, t: f32) {
    let c = view.to_screen(spot.pos) + vec2(0.0, -ROAD_LIFT);
    let stone = Color::new(0.30, 0.30, 0.36, 1.0);
    let rune = flask_color(spot);

    match spot.state {
        SpotState::Fresh | SpotState::Pending => {
            let pulse = 0.5 + 0.5 * (t * 3.0).sin();
            // A ring that keeps expanding outward and fading: a beacon you
            // can spot from the far end of the screen.
            let ripple = (t * 0.8).fract();
            gfx::ellipse_outline(
                c,
                14.0 + ripple * 22.0,
                7.0 + ripple * 11.0,
                2.0,
                gfx::with_alpha(rune, 0.9 * (1.0 - ripple)),
            );
            draw_rune_stone(c, stone, gfx::with_alpha(rune, 0.6 + 0.4 * pulse));
        }
        SpotState::Solved => {
            draw_rune_stone(c, stone, gfx::with_alpha(LAB_GREEN, 0.5));
            gfx::polyline(
                &[
                    c + vec2(-6.0, 0.0),
                    c + vec2(-1.5, 3.5),
                    c + vec2(7.0, -3.5),
                ],
                2.6,
                LAB_GREEN,
            );
        }
        SpotState::Missed => {
            gfx::ellipse(c, 14.0, 7.5, gfx::shade(stone, 0.8));
            gfx::polyline(
                &[
                    c + vec2(-8.0, -2.0),
                    c + vec2(-2.0, 1.0),
                    c + vec2(3.0, -2.5),
                    c + vec2(9.0, 1.5),
                ],
                1.2,
                INK,
            );
        }
    }
}

fn draw_rune_stone(c: Vec2, stone: Color, rune: Color) {
    gfx::ellipse(c + vec2(0.0, 1.5), 17.0, 9.0, INK);
    gfx::ellipse(c, 16.0, 8.5, stone);
    gfx::ellipse(c + vec2(-3.0, -2.0), 9.0, 3.5, gfx::shade(stone, 1.2));
    gfx::ellipse_outline(c, 11.0, 5.6, 1.4, rune);
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU;
        let d = vec2(a.cos() * 13.5, a.sin() * 7.0);
        gfx::line(c + d * 0.86, c + d * 1.04, 1.4, rune);
    }
}

/// The trigger itself: a potion flask hovering and bobbing over its rune
/// stone, glinting now and then.
fn draw_flask(view: &View, spot: &Spot, t: f32) {
    let ground = view.to_screen(spot.pos) + vec2(0.0, -ROAD_LIFT);
    let bob = (t * 2.4 + spot.pos.y * 0.01).sin();
    let base = ground + vec2(0.0, -10.0 - bob * 4.0);
    let color = flask_color(spot);

    gfx::ellipse(ground, 10.0 - bob * 1.5, 4.0, SHADOW);
    gfx::glow(
        base + vec2(0.0, -12.0),
        26.0,
        26.0,
        gfx::with_alpha(color, 0.30),
    );
    gfx::potion_flask(base, 1.0, color, 0.5 + 0.5 * bob, t, 1.0);

    let twinkle = (t * 3.1 + spot.pos.x).sin().max(0.0).powi(3);
    gfx::star(
        base + vec2(12.0, -34.0),
        3.0 + 4.0 * twinkle,
        1.0,
        4,
        0.0,
        1.0,
        gfx::with_alpha(WHITE, 0.3 + 0.7 * twinkle),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHUNKS: std::ops::Range<i64> = FIRST_CHUNK..300;

    #[test]
    fn scenery_never_lands_on_the_road() {
        for k in CHUNKS {
            for d in decor::generate_chunk(k, spot_for_chunk(k)) {
                if d.pos.y >= ROAD_SOUTH_END {
                    continue; // open ground below the gate
                }
                let gap = (d.pos.x - path_center(d.pos.y)).abs() - PATH_HALF_W;
                assert!(
                    gap > 10.0,
                    "chunk {k}: prop at {:?} is {gap}px from the road edge",
                    d.pos
                );
            }
        }
    }

    #[test]
    fn every_flask_is_reachable_and_on_the_road() {
        let reach = PATH_HALF_W - WALK_MARGIN;
        for k in CHUNKS {
            if let Some(p) = spot_for_chunk(k) {
                assert!(p.y < ROAD_START_Y, "flasks are on the road, not in the hub");
                assert!(
                    (p.x - path_center(p.y)).abs() <= reach,
                    "chunk {k}: flask at {p:?}"
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
    fn interact_reaches_only_nearby_fresh_flasks() {
        let mut world = World::new();
        let i = world
            .spots
            .iter()
            .position(|s| s.pos.y < 0.0)
            .expect("first flask is generated with the world");
        let at = world.spots[i].pos;

        assert_eq!(world.interactable_spot(at + vec2(30.0, 20.0)), Some(i));
        assert_eq!(world.interactable_spot(at + vec2(0.0, 200.0)), None);

        world.spots[i].state = SpotState::Solved;
        assert_eq!(world.interactable_spot(at), None);
    }

    #[test]
    fn clamp_keeps_the_player_on_the_road() {
        for y in (-20_000..ROAD_SOUTH_END as i32).step_by(37) {
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
            if let Some(Encounter { pos: p, .. }) = a {
                any += 1;
                assert!(p.y < ROAD_START_Y, "chunk {k}: zone in the hub");
                assert!(
                    spot_for_chunk(k).is_none(),
                    "chunk {k}: zone shares a flask's chunk"
                );
                assert_eq!(clamp_to_path(p), p, "chunk {k}: zone off the road");
            }
        }
        assert!(any > 50, "only {any} zones");
    }

    #[test]
    fn a_tide_titan_guards_every_500_m() {
        let bosses: Vec<u32> = CHUNKS
            .filter_map(encounter_for_chunk)
            .filter(|e| e.kind == MonsterKind::TideTitan)
            .inspect(|e| assert!(e.blocks, "the boss stands across the road"))
            .map(|e| ((ROAD_START_Y - e.pos.y) / PX_PER_METER) as u32)
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
            if let Some(Encounter { pos: p, kind, .. }) = encounter_for_chunk(k) {
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
    fn monsters_block_the_road_regularly_and_deterministically() {
        let blockers: Vec<(i64, Encounter)> = CHUNKS
            .filter_map(|k| encounter_for_chunk(k).map(|e| (k, e)))
            .filter(|(_, e)| e.blocks)
            .collect();
        assert!(blockers.len() > 40, "only {} blockers", blockers.len());
        let mut last_m = 0.0;
        for (k, e) in &blockers {
            assert_eq!(encounter_for_chunk(*k), Some(*e));
            assert!(e.pos.y < ROAD_START_Y, "chunk {k}: blocker in the hub");
            assert!(
                spot_for_chunk(*k).is_none(),
                "chunk {k}: shares a flask's chunk"
            );
            assert!(
                (e.pos.x - path_center(e.pos.y)).abs() < 1e-3,
                "chunk {k}: blocker off the middle of the road"
            );
            // One every ~80 m, never a long empty stretch.
            let m = (ROAD_START_Y - e.pos.y) / PX_PER_METER;
            assert!(
                m - last_m < 100.0,
                "chunk {k}: {} m since the last",
                m - last_m
            );
            last_m = m;
        }
        // The first one turns up early, so the player meets it soon.
        let first_m = (ROAD_START_Y - blockers[0].1.pos.y) / PX_PER_METER;
        assert!(first_m < 70.0, "first blocker at {first_m} m");
    }

    #[test]
    fn a_blocker_shuts_the_road_until_beaten() {
        let mut world = World::new();
        world.ensure_generated(-3_000.0);
        let i = world
            .zones
            .iter()
            .position(|z| z.blocks)
            .expect("a blocker within the first few chunks");
        let at = world.zones[i].pos;
        let line = at.y + BLOCK_GAP;

        // Walking north stops at the line; south of it is untouched.
        let past = vec2(at.x, line - 5.0);
        assert_eq!(world.clamp_blocked(past).y, line);
        let before = vec2(at.x + 20.0, line + 40.0);
        assert_eq!(world.clamp_blocked(before), before);
        // Blockers don't ambush, but can be challenged from the line.
        assert_eq!(world.encounter_at(at), None);
        assert_eq!(world.blocker_in_reach(vec2(at.x, line)), Some(i));
        assert_eq!(world.blocker_in_reach(vec2(at.x, line + 200.0)), None);

        world.zones[i].cleared = true;
        assert_eq!(world.clamp_blocked(past), past);
        assert_eq!(world.blocker_in_reach(vec2(at.x, line)), None);
    }

    #[test]
    fn hub_is_wide_dry_and_bounded() {
        for y in (ROAD_SOUTH_END as i32..3000).step_by(23) {
            for x in [-500.0, 0.0, 480.0, 960.0, 2000.0] {
                let p = clamp_to_path(vec2(x, y as f32));
                assert!(p.y <= SOUTH_LIMIT);
                // Never in the water, never off the world.
                assert!(p.x > bank_x(p.y) + 30.0, "wet at {p:?}");
                assert!(p.x < WORLD_W, "off the world at {p:?}");
            }
        }
        // Past the funnel the hub is much wider than the road.
        let (lo, hi) = walkable_x(PLAYER_START_Y);
        assert!(hi - lo > 3.0 * (PATH_HALF_W - WALK_MARGIN) * 2.0);
        // And the spawn point is walkable.
        let spawn = vec2(path_center(PLAYER_START_Y), PLAYER_START_Y);
        assert_eq!(clamp_to_path(spawn), spawn);
    }

    #[test]
    fn walkable_width_is_continuous_through_the_gate() {
        let mut prev = walkable_x(ROAD_SOUTH_END - 50.0);
        let mut y = ROAD_SOUTH_END - 50.0;
        while y < ROAD_SOUTH_END + HUB_FUNNEL + 50.0 {
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
