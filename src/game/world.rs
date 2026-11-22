//! The forest world (FR24): an old cobbled road that winds north (up the
//! screen) between a black lake on the left and the woods on the right, with
//! the mad scientist's manor across the water. It runs through a few regions
//! (`regions`) and ends at the castle's gate, `CASTLE_M` up the road (FR27).
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

use super::castle;
use super::decor::{self, Decoration};
use super::gfx::{self, INK, LAB_GREEN, LANTERN, MOON, SHADOW};
use super::manor;
use super::monster::{self, MonsterCanvas, MonsterKind, Pose, Species};
use super::regions;
use crate::content::MonsterSpec;
use macroquad::prelude::*;
use std::sync::Arc;

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
/// How far up the road the castle stands (FR27, the owner's answer: 1,500 m).
/// The road ends at its gate, so it is no longer endless.
pub const CASTLE_M: f32 = 1500.0;
/// World y of the castle gate: the northmost point the player can reach.
pub const CASTLE_GATE_Y: f32 = ROAD_START_Y - CASTLE_M * PX_PER_METER;
/// The player has arrived once they stand within this much of the gate line.
const CASTLE_ARRIVE_PX: f32 = 8.0;
/// The castle's curtain wall stands this far north of the gate line. Nothing
/// is placed, and nothing drawn but the castle, north of it.
pub const CASTLE_WALL_Y: f32 = CASTLE_GATE_Y - 40.0;
const _: () = assert!(
    CASTLE_WALL_Y < CASTLE_GATE_Y,
    "the wall stands beyond the gate line"
);
/// The last stretch of road before the gate holds no flask, monster or gear:
/// the player is nearly at the castle and nothing should be in the way.
const FORECOURT_PX: f32 = 480.0;
/// The doctor ducks into the castle once the player is this close to the gate.
pub const CASTLE_NEAR_PX: f32 = 380.0;
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

/// A safe stop (FR28) stands every this many meters up the road...
pub const STOP_EVERY_M: u32 = 250;
/// ...each one this many meters short of the round mark, so the ones at the
/// 500 m marks sit three chunks before that mark's Tide Titan and the player
/// can mend before the boss.
const STOP_BEFORE_M: u32 = 40;
/// Half the length of the stop's flat clearing along the road.
const STOP_FLAT_HALF: f32 = 80.0;
/// How far the road eases in and out of the clearing on each end.
const STOP_EASE: f32 = 70.0;
/// Half the stop's whole span: the clearing and its two eased ends. Inside it
/// the curse lifts, hearts mend faster and nothing can hurt the player. It
/// fits inside the stop's own chunk (half a chunk is 160 px).
pub const STOP_HALF_LEN: f32 = STOP_FLAT_HALF + STOP_EASE;
/// How far the clearing reaches out past the road's edge, on the woods side.
pub const STOP_EXTRA_W: f32 = 130.0;
/// The fence opens where the road has widened past this much (0 to 1).
const FENCE_OPEN_FROM: f32 = 0.1;
/// How close to the cottage's door E opens the shop.
pub const STOP_SHOP_REACH: f32 = 64.0;

/// A trail of gears (FR17) is tried in this share of the road's chunks. Those
/// with no room beside the flask or monster go without, so about a third end
/// up with one.
const GEAR_CHUNK_CHANCE: f32 = 0.75;
/// Gears in a trail sit this far apart along the road.
const GEAR_STEP: f32 = 30.0;
/// The player picks a gear up from this close.
pub const GEAR_PICKUP_R: f32 = 24.0;
/// A gear keeps this far from a flask on the road, and a monster's zone keeps
/// this much of the road's length clear of gears.
const GEAR_CLEAR: f32 = 40.0;

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
        return (c - reach, c + reach + stop_bulge(y) * STOP_EXTRA_W);
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

/// True once a player at `y` has reached the castle gate (FR27).
pub fn at_the_gate(y: f32) -> bool {
    y <= CASTLE_GATE_Y + CASTLE_ARRIVE_PX
}

/// True while a player at `y` is close enough to the castle for the doctor to
/// have gone inside.
pub fn near_the_castle(y: f32) -> bool {
    y < CASTLE_GATE_Y + CASTLE_NEAR_PX
}

/// True in the last stretch before the gate, where nothing is placed.
fn in_forecourt(y: f32) -> bool {
    y < CASTLE_GATE_Y + FORECOURT_PX
}

/// Keeps a position walkable: on the road north of the gate, anywhere on
/// the hub's ground south of it, never past the hub's south edge or the
/// castle's gate (FR27).
pub fn clamp_to_path(p: Vec2) -> Vec2 {
    let y = p.y.clamp(CASTLE_GATE_Y, SOUTH_LIMIT);
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

/// A safe stop (FR28): a lantern-lit clearing the road opens onto, with a
/// cottage, a fire and a shop. It sits on the woods side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SafeStop {
    /// Which stop this is up the road, from 1.
    pub n: u32,
    /// World y of the clearing's middle: the middle of its chunk.
    pub y: f32,
    /// Distance carved on its sign, in meters from the gate.
    pub meters: u32,
}

impl SafeStop {
    /// World position of the cottage's base.
    pub fn house(&self) -> Vec2 {
        let y = self.y - 26.0;
        vec2(path_center(y) + PATH_HALF_W + 96.0, y)
    }

    /// Where the player stands to use the shop: in front of the cottage.
    pub fn door(&self) -> Vec2 {
        self.house() + vec2(0.0, 18.0)
    }
}

/// Which stop, if any, stands in chunk `k`. Pure, like `spot_for_chunk`
/// (invariant 3): stop `n` is at `STOP_EVERY_M * n - STOP_BEFORE_M` meters, and
/// its clearing is centred in the chunk that holds that mark. Never in the hub.
pub fn safe_stop_for_chunk(k: i64) -> Option<SafeStop> {
    if k < 0 {
        return None;
    }
    let (north, south) = chunk_span(k);
    let d_south = (ROAD_START_Y - south) / PX_PER_METER;
    let d_north = (ROAD_START_Y - north) / PX_PER_METER;
    let n = (((d_south + STOP_BEFORE_M as f32) / STOP_EVERY_M as f32).ceil() as i64).max(1);
    let meters = STOP_EVERY_M as i64 * n - STOP_BEFORE_M as i64;
    let d = meters as f32;
    (d > d_south && d <= d_north && d + 20.0 < CASTLE_M).then_some(SafeStop {
        n: n as u32,
        y: north + CHUNK_H * 0.5,
        meters: meters as u32,
    })
}

/// The stop whose chunk holds `y`, if there is one.
pub fn stop_at(y: f32) -> Option<SafeStop> {
    safe_stop_for_chunk(chunk_of(y))
}

/// How far the road has opened onto a stop's clearing at `y`: 0 on the plain
/// road, 1 across the flat part, easing between (continuous, like the hub's
/// funnel).
pub fn stop_bulge(y: f32) -> f32 {
    match stop_at(y) {
        Some(s) => 1.0 - smoothstep(STOP_FLAT_HALF, STOP_HALF_LEN, (y - s.y).abs()),
        None => 0.0,
    }
}

/// True while `y` is inside a safe stop's span (FR28): the curse lifts, hearts
/// mend faster, and nothing can hurt the player.
pub fn in_safe_stop(y: f32) -> bool {
    stop_at(y).is_some_and(|s| (y - s.y).abs() <= STOP_HALF_LEN)
}

/// A stop's chunk and the two beside it hold no monster, so none is ever near
/// a clearing.
fn near_a_stop(k: i64) -> bool {
    (k - 1..=k + 1).any(|j| safe_stop_for_chunk(j).is_some())
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
        /// False when the next post is gone, for a stop's opening (FR28).
        rail: bool,
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
                rail,
            } => draw_post(view, *base, *next, *lantern, *rail, t),
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
    /// The built-in monster `encounter_for_chunk` placed here. It's who
    /// stands here unless `spec` says otherwise.
    pub kind: MonsterKind,
    /// A spec monster standing here instead (FR21), chosen by `pick_spec`
    /// from the world's pool. Never a boss.
    pub spec: Option<Arc<MonsterSpec>>,
    /// Stands on the road and blocks it (FR2), rather than lurking.
    pub blocks: bool,
    pub cleared: bool,
    /// Off-screen canvas for drawing a blocker on the road.
    canvas: MonsterCanvas,
}

impl EncounterZone {
    /// Who's here: the spec monster if there is one, else the built-in.
    pub fn species(&self) -> Species {
        match &self.spec {
            Some(s) => Species::Spec(s.clone()),
            None => Species::Builtin(self.kind),
        }
    }
}

/// The share of zones that get a spec monster instead of a built-in, when the
/// pool has one that's far enough up the road. Bosses never do.
pub const SPEC_SHARE: f32 = 0.4;

/// Which spec monster, if any, stands in the zone `encounter_for_chunk(k)`
/// placed. Pure in the chunk and the pool (invariant 3): the same chunk and
/// the same pool always give the same answer. `pool` is sorted by id.
///
/// A boss is never replaced. Otherwise about `SPEC_SHARE` of zones qualify,
/// and one is picked by hashing the chunk over the specs whose `min_m` the
/// zone's distance has reached. An empty pool, or nothing eligible yet, means
/// the built-in stays, exactly as before FR21.
pub fn pick_spec(k: i64, e: &Encounter, pool: &[Arc<MonsterSpec>]) -> Option<Arc<MonsterSpec>> {
    if pool.is_empty() || e.kind.info().boss {
        return None;
    }
    let h = |n: u64| gfx::hash01((k as u64).wrapping_mul(23).wrapping_add(n) ^ 0x5BEC);
    if h(1) >= SPEC_SHARE {
        return None;
    }
    let meters = ((ROAD_START_Y - e.pos.y) / PX_PER_METER).max(0.0) as u32;
    let eligible: Vec<&Arc<MonsterSpec>> =
        pool.iter().filter(|s| s.info().min_m <= meters).collect();
    if eligible.is_empty() {
        return None;
    }
    let i = ((h(2) * eligible.len() as f32) as usize).min(eligible.len() - 1);
    Some(eligible[i].clone())
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
    if k < 1 || k % 2 == 0 || near_a_stop(k) {
        return None;
    }
    // A Tide Titan guards each 500 m milestone: the first odd chunk at or
    // past it gets the boss, always, standing across the road.
    let (north, _) = chunk_span(k);
    let (_, prev_south) = chunk_span(k - 1);
    let milestone_px = BOSS_EVERY_M as f32 * PX_PER_METER;
    let lo_m = ((ROAD_START_Y - prev_south) / milestone_px).ceil().max(1.0) as i64;
    let at = ROAD_START_Y - lo_m as f32 * milestone_px;
    // No Titan at (or past) the castle: its gate is the road's end.
    let before_castle = (lo_m as f32 * BOSS_EVERY_M as f32) < CASTLE_M;
    if at <= prev_south && at > north && before_castle {
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
    if in_forecourt(y) {
        return None;
    }
    let meters = ((ROAD_START_Y - y) / PX_PER_METER).max(0.0) as u32;
    Some(Encounter {
        pos: vec2(path_center(y), y),
        kind: MonsterKind::pick(h(5), meters),
        blocks,
    })
}

/// Where a region starts on the road (FR27): its border sign.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionBorder {
    /// Index into `regions::REGIONS`.
    pub index: usize,
    pub from_m: u32,
    /// World y of the border.
    pub y: f32,
    /// Where the sign stands: beside the road, on the woods side.
    pub sign: Vec2,
}

/// The region borders that fall in chunk `k`: each region after the first
/// starts at a fixed distance, so this is pure (invariant 3).
pub fn region_borders_in_chunk(k: i64) -> Vec<RegionBorder> {
    let (north, south) = chunk_span(k);
    regions::REGIONS
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(index, r)| {
            let y = ROAD_START_Y - r.from_m as f32 * PX_PER_METER;
            (y > north && y <= south).then(|| RegionBorder {
                index,
                from_m: r.from_m,
                y,
                sign: vec2(path_center(y) + PATH_HALF_W + 34.0, y),
            })
        })
        .collect()
}

/// What the goalpost tracker (FR31) can point at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GoalKind {
    /// A potion flask still waiting on the road.
    Flask,
    /// A patch of ferns with a monster hiding in it (FR10).
    Patch,
    /// The distance marker stone at every `GOAL_MILESTONE_EVERY_M`.
    Milestone,
    /// A monster standing on the road, shutting it (FR2).
    Blocker,
    /// The Tide Titan at each 500 m mark.
    Titan,
    /// A safe stop (FR28): somewhere to rest.
    Stop,
    /// The border of a new region (FR27): a sign beside the road.
    Region,
    /// The castle gate, where the road ends (FR27).
    Castle,
}

impl GoalKind {
    /// Which goalpost wins when several are about the same distance away:
    /// the bigger event, so the tracker isn't always "flask in 12 m".
    fn rank(self) -> u8 {
        match self {
            GoalKind::Castle => 6,
            GoalKind::Titan => 5,
            GoalKind::Blocker => 4,
            GoalKind::Stop | GoalKind::Region => 3,
            GoalKind::Milestone => 2,
            GoalKind::Patch => 1,
            GoalKind::Flask => 0,
        }
    }
}

/// A thing up the road for the tracker to count down to (FR31).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Goalpost {
    pub kind: GoalKind,
    /// The world y at which the player has "arrived": where the flask can be
    /// opened, the ferns start the battle, or a blocker stops them. Counting
    /// down to here is what lets the readout reach 0 m on arrival.
    pub y: f32,
    /// Where the thing itself stands, to tell which one it is.
    pub at: Vec2,
    /// Distance carved on a milestone stone; 0 for everything else.
    pub meters: u32,
}

/// The stones the tracker counts down to: every this many meters. The road
/// has a stone every 50 m, which is too often to be a goal.
pub const GOAL_MILESTONE_EVERY_M: u32 = 100;
/// A higher-ranked goalpost beats the nearest one if it is at most this much
/// farther away.
const GOAL_RANK_SLACK_M: f32 = 25.0;
/// How many chunks north of the player `World::next_goalpost` looks: about
/// 100 m, always enough to find a flask or a patch.
const GOAL_SCAN_CHUNKS: i64 = 8;
/// How far past a flask's y the player is when it can be opened.
const FLASK_GOAL_REACH: f32 = 30.0;

/// Every goalpost in chunk `k`, from the same pure functions that place the
/// things themselves, so the tracker agrees with the world (invariant 3).
/// Includes ones the player has already dealt with; `World::next_goalpost`
/// filters those.
pub fn goalposts_in_chunk(k: i64) -> Vec<Goalpost> {
    let mut out = Vec::new();
    let (north, south) = chunk_span(k);
    // Milestone stones sit where `decor::generate_chunk` puts them.
    let step = GOAL_MILESTONE_EVERY_M;
    let mut m = (((ROAD_START_Y - south) / PX_PER_METER / step as f32)
        .ceil()
        .max(1.0) as u32)
        * step;
    loop {
        let y = ROAD_START_Y - m as f32 * PX_PER_METER;
        if y <= north || m as f32 >= CASTLE_M {
            break;
        }
        if y < south {
            out.push(Goalpost {
                kind: GoalKind::Milestone,
                y,
                at: vec2(path_center(y) - PATH_HALF_W - 20.0, y),
                meters: m,
            });
        }
        m += step;
    }
    for r in region_borders_in_chunk(k) {
        out.push(Goalpost {
            kind: GoalKind::Region,
            y: r.y,
            at: r.sign,
            meters: r.from_m,
        });
    }
    if chunk_of(CASTLE_GATE_Y) == k {
        out.push(Goalpost {
            kind: GoalKind::Castle,
            y: CASTLE_GATE_Y + CASTLE_ARRIVE_PX,
            at: vec2(path_center(CASTLE_GATE_Y), CASTLE_WALL_Y),
            meters: CASTLE_M as u32,
        });
    }
    if let Some(stop) = safe_stop_for_chunk(k) {
        // Arrived at the clearing's south edge, where the road has opened.
        out.push(Goalpost {
            kind: GoalKind::Stop,
            y: stop.y + STOP_FLAT_HALF,
            at: stop.house(),
            meters: stop.meters,
        });
    }
    if let Some(pos) = spot_for_chunk(k) {
        out.push(Goalpost {
            kind: GoalKind::Flask,
            y: pos.y + FLASK_GOAL_REACH,
            at: pos,
            meters: 0,
        });
    }
    if let Some(e) = encounter_for_chunk(k) {
        let (kind, y) = match (e.kind, e.blocks) {
            (MonsterKind::TideTitan, _) => (GoalKind::Titan, e.pos.y + BLOCK_GAP),
            (_, true) => (GoalKind::Blocker, e.pos.y + BLOCK_GAP),
            (_, false) => (GoalKind::Patch, e.pos.y + ZONE_HALF_H),
        };
        out.push(Goalpost {
            kind,
            y,
            at: e.pos,
            meters: 0,
        });
    }
    out
}

/// The goalpost the tracker should show for a player at `y`: of the ones
/// north of them, the nearest, unless a higher-ranked one is within
/// `GOAL_RANK_SLACK_M` of being as near. Pure, so it's testable on its own.
pub fn pick_goalpost(posts: &[Goalpost], y: f32) -> Option<Goalpost> {
    let ahead = || posts.iter().filter(|g| g.y < y);
    let nearest = ahead().map(|g| y - g.y).fold(f32::INFINITY, f32::min);
    let limit = nearest + GOAL_RANK_SLACK_M * PX_PER_METER;
    ahead()
        .filter(|g| y - g.y <= limit)
        // Highest rank first; among equals the nearer one (larger y).
        .max_by(|a, b| a.kind.rank().cmp(&b.kind.rank()).then(a.y.total_cmp(&b.y)))
        .copied()
}

/// The gears lying in chunk `k` (FR17): about `GEAR_CHUNK_CHANCE` of the road's
/// chunks hold a short trail of three to five along the road, sitting well
/// clear of the chunk's flask and monster. Pure, like `spot_for_chunk`
/// (invariant 3). None in the hub or at a safe stop.
pub fn gears_for_chunk(k: i64) -> Vec<Vec2> {
    if k < 1 || safe_stop_for_chunk(k).is_some() {
        return Vec::new();
    }
    let h = |n: u64| gfx::hash01((k as u64).wrapping_mul(17).wrapping_add(n) ^ 0x6EA5);
    if h(0) >= GEAR_CHUNK_CHANCE {
        return Vec::new();
    }
    let (north, south) = chunk_span(k);
    let count = 3 + (h(1) * 3.0) as usize;
    let len = (count - 1) as f32 * GEAR_STEP;
    let flask = spot_for_chunk(k);
    let zone = encounter_for_chunk(k);
    // Keep the whole trail 50 px inside the chunk, so a neighbour's flask
    // (60 px in) or monster (80 px in) is never near it. A chunk holds a flask or a zone, never both;
    // the trail goes on whichever side of it has room, or is left out.
    let (lo, hi) = (north + 50.0, south - 50.0 - len);
    let obstacle = flask
        .map(|f| (f.y, GEAR_CLEAR))
        .or(zone.map(|e| (e.pos.y, ZONE_HALF_H + GEAR_CLEAR)));
    let y0 = match obstacle {
        None => lo + h(2) * (hi - lo),
        Some((o, clear)) => {
            let before = (lo, o - clear - len);
            let after = (o + clear, hi);
            let mut sides: Vec<(f32, f32)> = [before, after]
                .into_iter()
                .filter(|(a, b)| b >= a)
                .collect();
            if h(4) < 0.5 {
                sides.reverse();
            }
            match sides.first() {
                Some(&(a, b)) => a + h(2) * (b - a),
                None => return Vec::new(),
            }
        }
    };
    let lean = (h(3) * 2.0 - 1.0) * (PATH_HALF_W - 40.0);
    (0..count)
        .map(|i| {
            let y = y0 + i as f32 * GEAR_STEP;
            vec2(path_center(y) + lean + 10.0 * (i as f32 * 1.3).sin(), y)
        })
        .filter(|g| !in_forecourt(g.y))
        .filter(|g| flask.is_none_or(|f| f.distance(*g) > GEAR_CLEAR))
        .filter(|g| zone.is_none_or(|e| (e.pos.y - g.y).abs() > ZONE_HALF_H + GEAR_CLEAR))
        .collect()
}

/// Something up the road that will need questions (FR32): a flask, or a
/// monster to fight.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Upcoming {
    /// `Flask`, or the kind of monster goalpost (`Patch`, `Blocker`, `Titan`).
    pub kind: GoalKind,
    /// The goalpost's y: where the player has "arrived".
    pub y: f32,
    /// Where the thing stands, to tell which one it is.
    pub at: Vec2,
    /// Questions the fight takes if every answer is right: 1 for a flask.
    pub hits: u32,
    /// Added to the player's level for this one's questions (0 for a flask).
    pub difficulty_bias: u32,
}

/// Every flask and monster within `range_px` north of `y`, nearest first.
/// Pure, from the same placement functions as the world and the goalpost
/// tracker (invariant 3), so it needs no generated chunks. It includes ones
/// the player has dealt with; `World::upcoming` filters those. Nothing in the
/// spawn hub: monsters start at chunk 1, and a flask in chunk 0 is the first.
pub fn upcoming_in(y: f32, range_px: f32) -> Vec<Upcoming> {
    let mut out = Vec::new();
    for k in chunk_of(y).max(0)..=chunk_of(y - range_px) {
        let encounter = encounter_for_chunk(k);
        for g in goalposts_in_chunk(k) {
            if g.y >= y || y - g.y > range_px {
                continue;
            }
            let (hits, difficulty_bias) = match g.kind {
                GoalKind::Flask => (1, 0),
                GoalKind::Patch | GoalKind::Blocker | GoalKind::Titan => {
                    match encounter.filter(|e| e.pos == g.at) {
                        Some(e) => {
                            let info = e.kind.info();
                            (info.hits, info.difficulty_bias)
                        }
                        None => continue,
                    }
                }
                GoalKind::Milestone | GoalKind::Stop | GoalKind::Region | GoalKind::Castle => {
                    continue
                }
            };
            out.push(Upcoming {
                kind: g.kind,
                y: g.y,
                at: g.at,
                hits,
                difficulty_bias,
            });
        }
    }
    out.sort_by(|a, b| b.y.total_cmp(&a.y));
    out
}

pub struct World {
    /// The spec monsters zones may hold, sorted by id: the roster as it was
    /// when this run started (FR21). Frozen for the run, so a run is a pure
    /// function of the chunk index and this pool.
    pool: Vec<Arc<MonsterSpec>>,
    /// `chunks[i]` holds the scenery for chunk `FIRST_CHUNK + i`.
    chunks: Vec<Vec<Decoration>>,
    pub spots: Vec<Spot>,
    pub zones: Vec<EncounterZone>,
    /// Every gear generated so far (FR17), with whether it's been picked up.
    pub gears: Vec<Gear>,
    /// Every safe stop generated so far (FR28), south to north.
    pub stops: Vec<SafeStop>,
}

/// A gear lying on the road (FR17): the forest's coin, dropped from the mad
/// scientist's lab.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gear {
    pub pos: Vec2,
    pub taken: bool,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self::with_pool(Vec::new())
    }

    /// A fresh world whose zones may hold the spec monsters in `pool` (sorted
    /// by id).
    pub fn with_pool(pool: Vec<Arc<MonsterSpec>>) -> Self {
        let mut world = Self {
            pool,
            chunks: Vec::new(),
            spots: Vec::new(),
            zones: Vec::new(),
            gears: Vec::new(),
            stops: Vec::new(),
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
            let stop = safe_stop_for_chunk(k);
            self.chunks.push(decor::generate_chunk(k, spot, stop));
            self.stops.extend(stop);
            self.gears.extend(
                gears_for_chunk(k)
                    .into_iter()
                    .map(|pos| Gear { pos, taken: false }),
            );
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
                    spec: pick_spec(k, &e, &self.pool),
                    blocks: e.blocks,
                    cleared: false,
                    canvas: MonsterCanvas::default(),
                });
            }
        }
    }

    /// Sets the pool and rebinds every zone from it. For the moment the
    /// run's roster is known (the watcher's first scan lands a frame or two
    /// after the world is built), before the player has been anywhere near a
    /// monster. After that the pool is frozen until the next run.
    pub fn set_pool(&mut self, pool: Vec<Arc<MonsterSpec>>) {
        self.pool = pool;
        for z in &mut self.zones {
            let k = chunk_of(z.pos.y);
            let e = Encounter {
                pos: z.pos,
                kind: z.kind,
                blocks: z.blocks,
            };
            z.spec = pick_spec(k, &e, &self.pool);
        }
    }

    /// Swaps in a newer version of a spec (an edit to its file) wherever the
    /// world holds it, so a reload shows on the road. The pool's membership
    /// doesn't change.
    pub fn respec(&mut self, spec: &Arc<MonsterSpec>) {
        let same = |s: &Arc<MonsterSpec>| s.id == spec.id;
        for s in self.pool.iter_mut().filter(|s| same(s)) {
            *s = spec.clone();
        }
        for z in &mut self.zones {
            if z.spec.as_ref().is_some_and(same) {
                z.spec = Some(spec.clone());
            }
        }
    }

    /// What the goalpost tracker (FR31) should point at for a player at `y`:
    /// the next thing up the road they haven't dealt with. Solved flasks and
    /// beaten monsters are skipped.
    pub fn next_goalpost(&self, y: f32) -> Option<Goalpost> {
        let first = chunk_of(y).max(FIRST_CHUNK);
        let posts: Vec<Goalpost> = (first..first + GOAL_SCAN_CHUNKS)
            .flat_map(goalposts_in_chunk)
            .filter(|g| !self.goal_done(g))
            .collect();
        pick_goalpost(&posts, y)
    }

    /// What's coming up the road that still needs a question (FR32): flasks
    /// not yet opened and monsters not yet beaten, within `range_px` north of
    /// `y`, nearest first.
    pub fn upcoming(&self, y: f32, range_px: f32) -> Vec<Upcoming> {
        upcoming_in(y, range_px)
            .into_iter()
            .map(|mut u| {
                // A zone that holds a spec monster plays by the spec's tier.
                let held = self.zones.iter().find(|z| z.pos == u.at);
                if let Some(spec) = held.and_then(|z| z.spec.as_ref()) {
                    let info = spec.info();
                    (u.hits, u.difficulty_bias) = (info.hits, info.difficulty_bias);
                }
                u
            })
            .filter(|u| {
                !self.goal_done(&Goalpost {
                    kind: u.kind,
                    y: u.y,
                    at: u.at,
                    meters: 0,
                })
            })
            .collect()
    }

    /// True once the player has dealt with `g`: opened the flask, or beaten
    /// the monster. Milestones are never done; they're just passed.
    fn goal_done(&self, g: &Goalpost) -> bool {
        match g.kind {
            GoalKind::Milestone | GoalKind::Stop | GoalKind::Region | GoalKind::Castle => false,
            GoalKind::Flask => self
                .spots
                .iter()
                .any(|s| s.pos == g.at && s.state != SpotState::Fresh),
            GoalKind::Patch | GoalKind::Blocker | GoalKind::Titan => {
                self.zones.iter().any(|z| z.pos == g.at && z.cleared)
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

    /// Picks up every gear within reach of `pos` (FR17) and returns how many,
    /// where each one was, for the sparkle.
    pub fn collect_gears(&mut self, pos: Vec2) -> Vec<Vec2> {
        let mut got = Vec::new();
        for g in self.gears.iter_mut().filter(|g| !g.taken) {
            if g.pos.distance(pos) < GEAR_PICKUP_R {
                g.taken = true;
                got.push(g.pos);
            }
        }
        got
    }

    /// The safe stop (FR28) whose cottage door the player at `pos` is close
    /// enough to use: what pressing E would open.
    pub fn stop_in_reach(&self, pos: Vec2) -> Option<SafeStop> {
        self.stops
            .iter()
            .find(|s| s.door().distance(pos) < STOP_SHOP_REACH)
            .copied()
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
    fn posts(view: &View) -> impl Iterator<Item = (Vec2, Vec2, bool, bool)> {
        let first = ROAD_SOUTH_END - 6.0;
        let j_min = ((first - (view.bottom() + 40.0)) / POST_SPACING)
            .ceil()
            .max(0.0) as i64;
        let j_max = ((first - (view.top() - 40.0)) / POST_SPACING).floor() as i64;
        let post_inset = PATH_HALF_W - 2.0;
        (j_min..=j_max)
            .flat_map(move |j| {
                let y = first - j as f32 * POST_SPACING;
                let ny = y - POST_SPACING;
                [-1.0f32, 1.0].map(|side| {
                    // The fence opens onto a stop's clearing (FR28), on the woods
                    // side: no post where the road has widened, and no rail
                    // running into the gap.
                    let open = |y: f32| side > 0.0 && stop_bulge(y) > FENCE_OPEN_FROM;
                    (
                        vec2(path_center(y) + side * post_inset, y),
                        vec2(path_center(ny) + side * post_inset, ny),
                        j % 3 == 1,
                        open(y),
                        open(ny),
                    )
                })
            })
            // The fence ends at the castle's wall (FR27).
            .filter_map(|(base, next, lantern, gone, next_gone)| {
                (!gone && base.y > CASTLE_WALL_Y).then_some((
                    base,
                    next,
                    lantern,
                    !next_gone && next.y > CASTLE_WALL_Y,
                ))
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
        items.extend(
            Self::posts(view).map(|(base, next, lantern, rail)| Tall::Post {
                base,
                next,
                lantern,
                rail,
            }),
        );
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
        // As the castle (the manor up close, FR27) rises over the top of the
        // screen, the far one melts away.
        let fade = smoothstep(CASTLE_WALL_Y - 120.0, CASTLE_WALL_Y + 520.0, cam);
        manor::draw_fading(vec2(x, y), scale, t, true, reduce_flashing, fade);
    }

    /// The castle at the road's end (FR27): a wall across the screen with a
    /// gate in the road's line, once the camera is near enough to see it.
    /// `open` is how far the portcullis has risen (0 shut, 1 open). Painted
    /// after the road and lights, before the depth-sorted pass.
    pub fn draw_castle(&self, view: &View, t: f32, open: f32, reduce_flashing: bool) {
        castle::draw(view, t, open, reduce_flashing);
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
        for stop in &self.stops {
            if stop.y + STOP_HALF_LEN > view.top() && stop.y - STOP_HALF_LEN < view.bottom() {
                draw_stop_pad(view, stop);
            }
        }
        for g in self.gears.iter().filter(|g| !g.taken) {
            if g.pos.y > view.top() - 30.0 && g.pos.y < view.bottom() + 30.0 {
                draw_gear(view, g.pos, t);
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
        for (base, _, lantern, _) in Self::posts(view) {
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
    if k < 0 || k % SPOT_EVERY_N_CHUNKS != 0 || safe_stop_for_chunk(k).is_some() {
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
    if in_forecourt(y) {
        return None;
    }
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
        &zone.species(),
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
fn draw_post(view: &View, base: Vec2, next: Vec2, lantern: bool, rail: bool, t: f32) {
    const POST_H: f32 = 28.0;
    let iron = Color::new(0.11, 0.11, 0.15, 1.0);
    let lift = vec2(0.0, -ROAD_LIFT);
    let s = view.to_screen(base) + lift;
    let top = s + vec2(0.0, -POST_H);
    let next_s = view.to_screen(next) + lift;

    for h in [POST_H - 5.0, 9.0].into_iter().filter(|_| rail) {
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

/// The brass of a gear (FR17), and its dark edge.
const BRASS: Color = Color::new(0.90, 0.68, 0.24, 1.0);
const BRASS_DARK: Color = Color::new(0.50, 0.33, 0.12, 1.0);

/// A gear lying on the road (FR17), bobbing and turning about its vertical
/// axis like a spinning coin. It animates on the game clock `t`.
fn draw_gear(view: &View, pos: Vec2, t: f32) {
    let phase = gfx::hash01((pos.x as i64 * 31 + pos.y as i64 * 17) as u64) * std::f32::consts::TAU;
    let bob = (t * 2.2 + phase).sin();
    let base = view.to_screen(pos) + vec2(0.0, -ROAD_LIFT);
    gfx::ground_shadow(base + vec2(0.0, 1.0), 9.0 - bob, 3.0);
    let c = base + vec2(0.0, -15.0 - 2.5 * bob);
    // Squash across the turn; never quite edge-on, so it never vanishes.
    let sx = (t * 2.6 + phase).cos().abs().max(0.2);
    gfx::glow(c, 20.0, 13.0, gfx::with_alpha(BRASS, 0.22));
    // Drawn 30% larger than its unit design so it reads from across the road.
    const U: f32 = 1.3;
    let at = |r: f32, a: f32| c + vec2(a.cos() * r * U * sx, a.sin() * r * U);
    // Eight teeth around the rim, then the body, the hub and the glint.
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        gfx::quad(
            at(6.0, a - 0.2),
            at(9.0, a - 0.14),
            at(9.0, a + 0.14),
            at(6.0, a + 0.2),
            BRASS_DARK,
        );
    }
    gfx::ellipse(c, (7.4 * sx + 0.8) * U, 7.6 * U, BRASS_DARK);
    gfx::ellipse(c, 6.6 * sx * U, 6.8 * U, BRASS);
    gfx::ellipse(c, 2.6 * sx * U, 2.8 * U, BRASS_DARK);
    gfx::ellipse(
        c + vec2(-2.0 * sx * U, -2.6 * U),
        1.6 * sx * U,
        1.1 * U,
        gfx::with_alpha(WHITE, 0.55),
    );
}

/// The clearing off the road at a safe stop (FR28): packed earth with a stone
/// edging, laid over the road's curb, widening and narrowing with
/// `stop_bulge` so it joins the road without a seam.
fn draw_stop_pad(view: &View, stop: &SafeStop) {
    let lift = vec2(0.0, -ROAD_LIFT);
    let earth = Color::new(0.24, 0.20, 0.17, 1.0);
    let edging = Color::new(0.30, 0.31, 0.36, 1.0);
    let p = |x: f32, y: f32| view.to_screen(vec2(x, y));
    const ROW: f32 = 6.0;
    let rows = (STOP_HALF_LEN * 2.0 / ROW) as i32;
    let edge = |y: f32| path_center(y) + PATH_HALF_W + STOP_EXTRA_W * stop_bulge(y);
    let inner = |y: f32| path_center(y) + PATH_HALF_W - 8.0;
    for pass in 0..3 {
        for i in 0..rows {
            let y0 = stop.y - STOP_HALF_LEN + i as f32 * ROW;
            let y1 = y0 + ROW;
            if edge(y0) - inner(y0) < 10.0 && edge(y1) - inner(y1) < 10.0 {
                continue;
            }
            let (a, b, c, d) = (
                p(inner(y0), y0),
                p(edge(y0), y0),
                p(edge(y1), y1),
                p(inner(y1), y1),
            );
            match pass {
                // The cast shadow, down and to the right.
                0 => gfx::quad(
                    a + vec2(8.0, 4.0),
                    b + vec2(8.0, 4.0),
                    c + vec2(8.0, 4.0),
                    d + vec2(8.0, 4.0),
                    gfx::with_alpha(SHADOW, 0.45),
                ),
                1 => gfx::quad(a + lift, b + lift, c + lift, d + lift, earth),
                // The edging along the outer side.
                _ => gfx::quad(
                    b + lift - vec2(4.0, 0.0),
                    b + lift,
                    c + lift,
                    c + lift - vec2(4.0, 0.0),
                    edging,
                ),
            }
        }
    }
    // Pebbles in the earth, fixed to the stop.
    for i in 0..26u64 {
        let h = |n: u64| gfx::hash01(stop.n as u64 * 977 + i * 13 + n);
        let y = stop.y - STOP_FLAT_HALF + h(1) * STOP_FLAT_HALF * 2.0;
        let x = path_center(y) + PATH_HALF_W + 8.0 + h(2) * (STOP_EXTRA_W - 16.0);
        let c = p(x, y) + lift;
        gfx::ellipse(
            c,
            1.6 + h(3) * 1.6,
            1.1 + h(3),
            gfx::with_alpha(edging, 0.7),
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

    /// Every chunk of the road, a little past the castle's.
    const CHUNKS: std::ops::Range<i64> = FIRST_CHUNK..115;

    fn spec(id: &str, tier: u32) -> Arc<MonsterSpec> {
        Arc::new(
            crate::content::monster_spec::parse(&format!(
                r##"{{"id":"{id}","name":"{id}","body":{{"shape":"blob","w":40,"h":30}},
                "palette":{{"main":"#88cc66","accent":"#ffcc44","belly":"#ddeebb"}},
                "eyes":{{"count":2,"size":4}},"tier":{tier}}}"##
            ))
            .unwrap(),
        )
    }

    fn pool() -> Vec<Arc<MonsterSpec>> {
        vec![
            spec("a-one", 1),
            spec("b-two", 2),
            spec("c-three", 3),
            spec("d-four", 4),
        ]
    }

    #[test]
    fn spec_monsters_take_their_share_of_zones_and_never_a_boss() {
        let pool = pool();
        let (mut zones, mut specs) = (0, 0);
        for k in 1..400 {
            let Some(e) = encounter_for_chunk(k) else {
                continue;
            };
            zones += 1;
            if let Some(s) = pick_spec(k, &e, &pool) {
                specs += 1;
                assert!(!e.kind.info().boss, "chunk {k}: a boss was replaced");
                let meters = ((ROAD_START_Y - e.pos.y) / PX_PER_METER) as u32;
                assert!(s.info().min_m <= meters, "{} too early at {meters} m", s.id);
            }
        }
        let share = specs as f32 / zones as f32;
        assert!((0.25..0.5).contains(&share), "{specs} of {zones} zones");
    }

    #[test]
    fn an_empty_pool_changes_nothing() {
        for k in 1..100 {
            if let Some(e) = encounter_for_chunk(k) {
                assert!(pick_spec(k, &e, &[]).is_none());
            }
        }
        let mut w = World::with_pool(Vec::new());
        w.ensure_generated(-8000.0);
        assert!(w.zones.iter().all(|z| z.spec.is_none()));
        assert!(w
            .zones
            .iter()
            .all(|z| matches!(z.species(), Species::Builtin(k) if k == z.kind)));
    }

    #[test]
    fn the_same_pool_gives_the_same_zones_every_time_and_placement_is_untouched() {
        let build = |pool: Vec<Arc<MonsterSpec>>| {
            let mut w = World::with_pool(pool);
            w.ensure_generated(-12_000.0);
            w
        };
        let (a, b, plain) = (build(pool()), build(pool()), build(Vec::new()));
        let held = |w: &World| -> Vec<Option<String>> {
            w.zones
                .iter()
                .map(|z| z.spec.as_ref().map(|s| s.id.clone()))
                .collect()
        };
        assert_eq!(held(&a), held(&b));
        assert!(held(&a).iter().any(Option::is_some));
        // Where zones are, which built-in they'd hold and whether they block
        // never depend on the pool (invariant 3).
        let layout = |w: &World| -> Vec<(Vec2, MonsterKind, bool)> {
            w.zones.iter().map(|z| (z.pos, z.kind, z.blocks)).collect()
        };
        assert_eq!(layout(&a), layout(&plain));
    }

    #[test]
    fn rebinding_a_built_world_matches_building_it_with_the_pool() {
        let mut late = World::new();
        late.ensure_generated(-9000.0);
        late.set_pool(pool());
        let mut fresh = World::with_pool(pool());
        fresh.ensure_generated(-9000.0);
        let held = |w: &World| -> Vec<Option<String>> {
            w.zones
                .iter()
                .map(|z| z.spec.as_ref().map(|s| s.id.clone()))
                .collect()
        };
        assert_eq!(held(&late), held(&fresh));
    }

    #[test]
    fn an_edited_spec_shows_in_the_zones_that_hold_it() {
        let mut w = World::with_pool(pool());
        w.ensure_generated(-12_000.0);
        let z = w.zones.iter().position(|z| z.spec.is_some()).unwrap();
        let id = w.zones[z].spec.as_ref().unwrap().id.clone();
        let edited = spec(&id, 4);
        w.respec(&edited);
        assert!(Arc::ptr_eq(w.zones[z].spec.as_ref().unwrap(), &edited));
        // A spec that was never in the pool doesn't join it.
        let before = w.pool.len();
        w.respec(&spec("newcomer", 1));
        assert_eq!(w.pool.len(), before);
    }

    #[test]
    fn upcoming_uses_a_held_specs_tier() {
        let mut w = World::with_pool(vec![spec("tough", 3)]);
        w.ensure_generated(-40_000.0);
        let z = w
            .zones
            .iter()
            .find(|z| z.spec.is_some() && !z.cleared)
            .unwrap();
        let u = w
            .upcoming(z.pos.y + 200.0, 600.0)
            .into_iter()
            .find(|u| u.at == z.pos)
            .unwrap();
        let info = z.spec.as_ref().unwrap().info();
        assert_eq!(
            (u.hits, u.difficulty_bias),
            (info.hits, info.difficulty_bias)
        );
    }

    #[test]
    fn upcoming_is_the_nearest_first_and_inside_the_range() {
        for start in [PLAYER_START_Y, -500.0, -4000.0, -25_000.0] {
            let ups = upcoming_in(start, 900.0);
            for u in &ups {
                assert!(u.y < start && start - u.y <= 900.0, "{u:?} from {start}");
                assert!(u.hits >= 1);
            }
            assert!(ups.windows(2).all(|w| w[0].y >= w[1].y), "nearest first");
        }
    }

    #[test]
    fn upcoming_matches_where_the_world_puts_things() {
        let mut found = (0, 0);
        for k in 1..200 {
            let (north, south) = chunk_span(k);
            let ups = upcoming_in(south, south - north);
            for u in ups {
                match u.kind {
                    GoalKind::Flask => {
                        found.0 += 1;
                        assert!(spot_for_chunk(chunk_of(u.at.y)).is_some_and(|p| p == u.at));
                    }
                    _ => {
                        found.1 += 1;
                        assert!(
                            encounter_for_chunk(chunk_of(u.at.y)).is_some_and(|e| e.pos == u.at)
                        );
                    }
                }
            }
        }
        assert!(found.0 > 20 && found.1 > 20, "{found:?}");
    }

    #[test]
    fn nothing_is_upcoming_in_the_hub_but_the_first_flask() {
        // From the start of the hub, 900 px reaches chunk 0's flask and chunk
        // 1's first monster. Nothing is placed in the hub itself.
        let ups = upcoming_in(PLAYER_START_Y, 900.0);
        assert!(ups.iter().all(|u| u.y < 0.0), "{ups:?}");
        assert!(ups.iter().any(|u| u.kind == GoalKind::Flask));
        assert!(upcoming_in(HUB_SOUTH, 400.0).is_empty());
    }

    #[test]
    fn a_monsters_hits_and_bias_come_from_its_info() {
        let ups = upcoming_in(0.0, 20_000.0);
        let titan = ups.iter().find(|u| u.kind == GoalKind::Titan).unwrap();
        let info = MonsterKind::TideTitan.info();
        assert_eq!(
            (titan.hits, titan.difficulty_bias),
            (info.hits, info.difficulty_bias)
        );
    }

    #[test]
    fn dealt_with_things_are_not_upcoming() {
        let mut w = World::new();
        w.ensure_generated(-6000.0);
        let before = w.upcoming(-100.0, 4000.0).len();
        assert!(before > 2);
        let i = w.spots.iter().position(|s| s.pos.y < -200.0).unwrap();
        w.spots[i].state = SpotState::Solved;
        let z = w
            .zones
            .iter()
            .position(|z| z.pos.y < -200.0 && z.pos.y > -4000.0)
            .unwrap();
        w.zones[z].cleared = true;
        assert_eq!(w.upcoming(-100.0, 4000.0).len(), before - 2);
    }

    #[test]
    fn scenery_never_lands_on_the_road() {
        for k in CHUNKS {
            for d in decor::generate_chunk(k, spot_for_chunk(k), safe_stop_for_chunk(k)) {
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
            let a: Vec<Vec2> = decor::generate_chunk(k, spot_for_chunk(k), safe_stop_for_chunk(k))
                .iter()
                .map(|d| d.pos)
                .collect();
            let b: Vec<Vec2> = decor::generate_chunk(k, spot_for_chunk(k), safe_stop_for_chunk(k))
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
                // The road's width, plus the clearing where a safe stop opens it.
                let reach = PATH_HALF_W - WALK_MARGIN;
                let dx = p.x - path_center(p.y);
                assert!(dx >= -reach - 1e-3);
                assert!(dx <= reach + stop_bulge(p.y) * STOP_EXTRA_W + 1e-3);
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
        assert!(any > 30, "only {any} zones");
    }

    #[test]
    fn a_tide_titan_guards_every_500_m() {
        let bosses: Vec<u32> = CHUNKS
            .filter_map(encounter_for_chunk)
            .filter(|e| e.kind == MonsterKind::TideTitan)
            .inspect(|e| assert!(e.blocks, "the boss stands across the road"))
            .map(|e| ((ROAD_START_Y - e.pos.y) / PX_PER_METER) as u32)
            .collect();
        // One boss per milestone, each just past it, and none at the castle's
        // gate (1,500 m), where the road ends.
        assert_eq!(bosses.len(), 2, "{bosses:?}");
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
                } else if m > 1000 {
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
        assert!(blockers.len() > 15, "only {} blockers", blockers.len());
        let mut last_m = 0.0;
        let mut last_k = 0;
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
            // One every ~80 m, never a long empty stretch, unless a safe stop
            // (FR28) took one of the chunks in between.
            let m = (ROAD_START_Y - e.pos.y) / PX_PER_METER;
            let stopped = (last_k..*k).any(|j| safe_stop_for_chunk(j).is_some());
            assert!(
                m - last_m < 100.0 || (stopped && m - last_m < 180.0),
                "chunk {k}: {} m since the last",
                m - last_m
            );
            last_m = m;
            last_k = *k;
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

    // ---- the castle and the regions (FR27) ----

    #[test]
    fn the_road_ends_at_the_castle_gate() {
        let x = path_center(CASTLE_GATE_Y);
        // No walking past it, however far the push.
        assert_eq!(
            clamp_to_path(vec2(x, CASTLE_GATE_Y - 900.0)).y,
            CASTLE_GATE_Y
        );
        assert_eq!(clamp_to_path(vec2(x, CASTLE_GATE_Y)).y, CASTLE_GATE_Y);
        // Short of it the road is as it was.
        let y = CASTLE_GATE_Y + 500.0;
        assert_eq!(clamp_to_path(vec2(x, y)).y, y);
        // The gate is 1,500 m up the road, and standing at it is arriving.
        assert!(((ROAD_START_Y - CASTLE_GATE_Y) / PX_PER_METER - CASTLE_M).abs() < 1e-3);
        assert!(at_the_gate(CASTLE_GATE_Y));
        assert!(at_the_gate(clamp_to_path(vec2(x, CASTLE_GATE_Y - 50.0)).y));
        assert!(!at_the_gate(CASTLE_GATE_Y + 100.0));
    }

    #[test]
    fn nothing_is_placed_in_the_forecourt_or_past_the_gate() {
        for k in CHUNKS {
            if let Some(f) = spot_for_chunk(k) {
                assert!(!in_forecourt(f.y), "chunk {k}: a flask at {}", f.y);
            }
            if let Some(e) = encounter_for_chunk(k) {
                assert!(
                    !in_forecourt(e.pos.y),
                    "chunk {k}: a monster at {}",
                    e.pos.y
                );
            }
            for g in gears_for_chunk(k) {
                assert!(!in_forecourt(g.y), "chunk {k}: a gear at {}", g.y);
            }
            if let Some(s) = safe_stop_for_chunk(k) {
                assert!(
                    (s.meters as f32) < CASTLE_M,
                    "chunk {k}: a stop at {} m",
                    s.meters
                );
            }
        }
        // The forecourt isn't the whole road: the stretch before it is as busy as ever.
        assert!(in_forecourt(CASTLE_GATE_Y + 100.0));
        assert!(!in_forecourt(CASTLE_GATE_Y + 3000.0));
    }

    #[test]
    fn the_castle_gate_is_a_goalpost_that_outranks_the_rest() {
        let k = chunk_of(CASTLE_GATE_Y);
        let posts = goalposts_in_chunk(k);
        let castle: Vec<_> = posts
            .iter()
            .filter(|g| g.kind == GoalKind::Castle)
            .collect();
        assert_eq!(castle.len(), 1, "{posts:?}");
        // Counting down reaches 0 m exactly when the player has arrived.
        assert!(at_the_gate(castle[0].y));
        // No other chunk has one, and no milestone stone stands at the gate.
        let all: Vec<_> = CHUNKS.flat_map(goalposts_in_chunk).collect();
        assert_eq!(all.iter().filter(|g| g.kind == GoalKind::Castle).count(), 1);
        assert!(all
            .iter()
            .filter(|g| g.kind == GoalKind::Milestone)
            .all(|g| (g.meters as f32) < CASTLE_M));
        assert!(GoalKind::Castle.rank() > GoalKind::Titan.rank());
    }

    #[test]
    fn each_region_border_is_a_goalpost_once() {
        let all: Vec<_> = CHUNKS.flat_map(goalposts_in_chunk).collect();
        for r in regions::REGIONS.iter().skip(1) {
            let n = all
                .iter()
                .filter(|g| g.kind == GoalKind::Region && g.meters == r.from_m)
                .count();
            assert_eq!(n, 1, "{} has {n} goalposts", r.name);
        }
        // The first region starts at the gate, which is no border.
        assert!(all
            .iter()
            .all(|g| !(g.kind == GoalKind::Region && g.meters == 0)));
    }

    #[test]
    fn region_borders_keep_clear_of_stops() {
        for k in CHUNKS {
            for b in region_borders_in_chunk(k) {
                assert!(!near_a_stop(k), "{}: a stop beside the border", b.from_m);
                assert!(b.y > chunk_span(k).0 && b.y <= chunk_span(k).1);
                assert!(
                    b.sign.x > path_center(b.y) + PATH_HALF_W,
                    "the sign is on the road"
                );
            }
        }
    }

    #[test]
    fn the_doctor_goes_inside_as_the_castle_nears() {
        assert!(!near_the_castle(ROAD_START_Y - 500.0 * PX_PER_METER));
        assert!(near_the_castle(CASTLE_GATE_Y + 100.0));
        // They're gone well before the gate, and not before the last safe stop.
        let stop = safe_stop_for_chunk(chunk_of(ROAD_START_Y - 1460.0 * PX_PER_METER)).unwrap();
        assert!(!near_the_castle(stop.y));
    }

    // ---- safe stops (FR28) ----

    fn stops() -> Vec<(i64, SafeStop)> {
        CHUNKS
            .filter_map(|k| safe_stop_for_chunk(k).map(|s| (k, s)))
            .collect()
    }

    #[test]
    fn stops_come_every_250_m_from_the_first_and_are_deterministic() {
        let all = stops();
        // 210, 460, 710, 960, 1210 and 1460 m: none at or past the castle.
        assert_eq!(all.len(), 6, "{all:?}");
        for (i, (k, s)) in all.iter().enumerate() {
            assert_eq!(safe_stop_for_chunk(*k), Some(*s));
            assert_eq!(s.n as usize, i + 1, "no stop skipped");
            assert_eq!(s.meters, STOP_EVERY_M * s.n - STOP_BEFORE_M);
            let (north, south) = chunk_span(*k);
            assert!(s.y > north && s.y < south);
            let m = (ROAD_START_Y - s.y) / PX_PER_METER;
            assert!((m - s.meters as f32).abs() < 8.0, "stop {} at {m} m", s.n);
        }
        assert_eq!(
            all[0].1.meters, 210,
            "the first is a short walk up the road"
        );
    }

    #[test]
    fn nothing_hostile_shares_a_stops_chunk_or_the_ones_beside_it() {
        for (k, s) in stops() {
            assert!(
                spot_for_chunk(k).is_none(),
                "stop {}: a flask is in its chunk",
                s.n
            );
            for j in k - 1..=k + 1 {
                assert!(
                    encounter_for_chunk(j).is_none(),
                    "stop {}: a zone at {j}",
                    s.n
                );
            }
            assert!(gears_for_chunk(k).is_empty());
            // No chunk with a stop is in the hub.
            assert!(s.y < ROAD_START_Y);
        }
    }

    #[test]
    fn the_stop_before_each_500_m_titan_leaves_room_to_heal() {
        let titans: Vec<i64> = CHUNKS
            .filter(|&k| encounter_for_chunk(k).is_some_and(|e| e.kind == MonsterKind::TideTitan))
            .collect();
        assert_eq!(titans.len(), 2, "the Titans at 500 and 1,000 m");
        for t in titans {
            let before = (t - 6..t)
                .filter(|&k| safe_stop_for_chunk(k).is_some())
                .count();
            assert_eq!(
                before, 1,
                "one stop in the six chunks before the Titan at {t}"
            );
            let near = (t - 2..t).any(|k| safe_stop_for_chunk(k).is_some());
            assert!(!near, "the stop isn't right against the Titan at {t}");
        }
    }

    #[test]
    fn the_road_opens_onto_the_clearing_without_a_seam() {
        let (_, s) = stops()[0];
        // Plain road away from a stop.
        let far = s.y + 400.0;
        assert_eq!(stop_bulge(far), 0.0);
        let reach = PATH_HALF_W - WALK_MARGIN;
        let (lo, hi) = walkable_x(far);
        assert!((hi - lo - 2.0 * reach).abs() < 1e-3);
        // Fully open across the flat part, only on the woods side.
        assert_eq!(stop_bulge(s.y), 1.0);
        let (lo, hi) = walkable_x(s.y);
        let c = path_center(s.y);
        assert!((lo - (c - reach)).abs() < 1e-3);
        assert!((hi - (c + reach + STOP_EXTRA_W)).abs() < 1e-3);
        // Continuous all the way through (no step bigger than a pixel or two).
        let mut prev = walkable_x(s.y + STOP_HALF_LEN + 60.0);
        let mut y = s.y + STOP_HALF_LEN + 60.0;
        while y > s.y - STOP_HALF_LEN - 60.0 {
            y -= 1.0;
            let w = walkable_x(y);
            assert!((w.1 - prev.1).abs() < 4.0, "right edge jumps at {y}");
            assert!((w.0 - prev.0).abs() < 4.0, "left edge jumps at {y}");
            prev = w;
        }
    }

    #[test]
    fn a_stops_span_is_safe_and_the_road_either_side_is_not() {
        let (k, s) = stops()[0];
        assert!(in_safe_stop(s.y));
        assert!(in_safe_stop(s.y + STOP_HALF_LEN - 1.0));
        assert!(!in_safe_stop(s.y + STOP_HALF_LEN + 1.0));
        assert!(!in_safe_stop(chunk_span(k + 2).0 + 100.0));
        assert!(
            !in_safe_stop(PLAYER_START_Y),
            "the hub is its own safe place"
        );
        // The whole span fits inside the stop's own chunk.
        let (north, south) = chunk_span(k);
        assert!(s.y - STOP_HALF_LEN > north && s.y + STOP_HALF_LEN < south);
    }

    #[test]
    fn the_cottage_door_is_reachable_and_opens_the_shop() {
        let mut w = World::new();
        w.ensure_generated(-6_000.0);
        let s = w.stops[0];
        let door = s.door();
        let (lo, hi) = walkable_x(door.y);
        assert!(
            door.x > lo && door.x < hi,
            "the door is inside the clearing"
        );
        assert_eq!(clamp_to_path(door), door, "and the player can stand there");
        assert_eq!(w.stop_in_reach(door), Some(s));
        assert_eq!(
            w.stop_in_reach(door + vec2(0.0, STOP_SHOP_REACH + 10.0)),
            None
        );
        assert_eq!(w.stop_in_reach(vec2(path_center(0.0), 0.0)), None);
    }

    #[test]
    fn a_stop_is_a_goalpost_and_never_done() {
        let (k, s) = stops()[0];
        let posts = goalposts_in_chunk(k);
        let g = posts
            .iter()
            .find(|g| g.kind == GoalKind::Stop)
            .expect("a Stop goalpost");
        assert_eq!(g.meters, s.meters);
        assert_eq!(g.at, s.house());
        assert!(g.y > s.y, "arrived at the clearing's south edge");
        let w = World::new();
        assert!(!w.goal_done(g));
        assert!(upcoming_in(g.y + 100.0, 400.0)
            .iter()
            .all(|u| u.kind != GoalKind::Stop));
    }

    // ---- gears (FR17) ----

    #[test]
    fn gears_lie_on_the_road_deterministically_and_clear_of_flasks_and_monsters() {
        let mut total = 0;
        let mut chunks_with = 0;
        for k in CHUNKS {
            let a = gears_for_chunk(k);
            assert_eq!(a, gears_for_chunk(k));
            if !a.is_empty() {
                chunks_with += 1;
            }
            let (north, south) = chunk_span(k);
            for g in &a {
                total += 1;
                assert!(k >= 1, "none in the hub");
                assert!(g.y > north && g.y < south);
                assert!(
                    (g.x - path_center(g.y)).abs() <= PATH_HALF_W - WALK_MARGIN,
                    "chunk {k}: gear {g:?} is off the road"
                );
                if let Some(f) = spot_for_chunk(k) {
                    assert!(f.distance(*g) > GEAR_CLEAR, "chunk {k}: gear on a flask");
                }
                if let Some(e) = encounter_for_chunk(k) {
                    assert!(
                        (e.pos.y - g.y).abs() > ZONE_HALF_H,
                        "chunk {k}: gear in a zone"
                    );
                }
            }
        }
        let n = CHUNKS.count() as f32;
        let share = chunks_with as f32 / n;
        assert!((0.2..0.5).contains(&share), "{share} of chunks hold gears");
        assert!(total > 60);
    }

    #[test]
    fn a_gear_is_picked_up_once_from_close_by() {
        let mut w = World::new();
        w.ensure_generated(-6_000.0);
        let first = w.gears[0];
        assert!(!first.taken);
        assert!(w
            .collect_gears(first.pos + vec2(GEAR_PICKUP_R + 20.0, 0.0))
            .is_empty());
        let got = w.collect_gears(first.pos + vec2(GEAR_PICKUP_R - 8.0, 0.0));
        assert!(got.contains(&first.pos));
        assert!(w.gears[0].taken);
        assert!(w.collect_gears(first.pos).is_empty(), "never twice");
    }

    #[test]
    fn a_new_world_has_all_its_gears_again() {
        let mut a = World::new();
        a.ensure_generated(-6_000.0);
        a.collect_gears(a.gears[0].pos);
        let mut b = World::new();
        b.ensure_generated(-6_000.0);
        assert!(b.gears.iter().all(|g| !g.taken));
        assert_eq!(a.gears.len(), b.gears.len());
    }

    // ---- goalposts (FR31) ----

    fn post(kind: GoalKind, y: f32) -> Goalpost {
        Goalpost {
            kind,
            y,
            at: vec2(0.0, y),
            meters: 0,
        }
    }

    #[test]
    fn goalposts_are_deterministic_and_stand_in_their_chunk() {
        for k in FIRST_CHUNK..300 {
            let a = goalposts_in_chunk(k);
            assert_eq!(a, goalposts_in_chunk(k));
            let (north, south) = chunk_span(k);
            for g in &a {
                assert!(g.at.y > north && g.at.y < south, "{k}: {g:?}");
                assert!(g.y >= g.at.y, "arrive at or south of the thing: {g:?}");
            }
        }
    }

    #[test]
    fn goalposts_cover_milestones_bosses_and_blockers() {
        let all: Vec<Goalpost> = (FIRST_CHUNK..300).flat_map(goalposts_in_chunk).collect();
        let stones: Vec<u32> = all
            .iter()
            .filter(|g| g.kind == GoalKind::Milestone)
            .map(|g| g.meters)
            .collect();
        assert_eq!(stones[0], GOAL_MILESTONE_EVERY_M);
        assert!(
            stones
                .windows(2)
                .all(|w| w[1] == w[0] + GOAL_MILESTONE_EVERY_M),
            "{stones:?}"
        );
        for g in all.iter().filter(|g| g.kind == GoalKind::Milestone) {
            assert!((ROAD_START_Y - g.y - g.meters as f32 * PX_PER_METER).abs() < 1e-2);
        }
        for kind in [
            GoalKind::Titan,
            GoalKind::Blocker,
            GoalKind::Patch,
            GoalKind::Flask,
        ] {
            assert!(all.iter().any(|g| g.kind == kind), "{kind:?}");
        }
    }

    #[test]
    fn nothing_hostile_is_a_goalpost_in_the_hub() {
        for k in FIRST_CHUNK..1 {
            for g in goalposts_in_chunk(k) {
                assert!(
                    !matches!(
                        g.kind,
                        GoalKind::Patch | GoalKind::Blocker | GoalKind::Titan
                    ),
                    "{g:?}"
                );
            }
        }
    }

    #[test]
    fn pick_goalpost_is_always_north_and_takes_the_nearest() {
        let y = -1000.0;
        let posts = [
            post(GoalKind::Flask, -900.0),  // behind the player
            post(GoalKind::Flask, -1000.0), // level: already reached
            post(GoalKind::Flask, -1300.0),
            post(GoalKind::Flask, -1600.0),
        ];
        assert_eq!(pick_goalpost(&posts, y).unwrap().y, -1300.0);
        assert_eq!(pick_goalpost(&[], y), None);
        assert_eq!(pick_goalpost(&posts[..2], y), None);
    }

    #[test]
    fn pick_goalpost_ranks_the_bigger_event_first() {
        let m = |n: f32| -n * PX_PER_METER;
        // A Titan 20 m farther than a flask beats it; 40 m farther doesn't.
        let flask = post(GoalKind::Flask, m(10.0));
        let near_titan = post(GoalKind::Titan, m(30.0));
        let far_titan = post(GoalKind::Titan, m(50.0));
        assert_eq!(
            pick_goalpost(&[flask, near_titan], 0.0).unwrap().kind,
            GoalKind::Titan
        );
        assert_eq!(
            pick_goalpost(&[flask, far_titan], 0.0).unwrap().kind,
            GoalKind::Flask
        );
        // Equal rank: the nearer.
        let a = post(GoalKind::Patch, m(20.0));
        let b = post(GoalKind::Patch, m(15.0));
        assert_eq!(pick_goalpost(&[a, b], 0.0).unwrap().y, m(15.0));
    }

    #[test]
    fn next_goalpost_skips_flasks_and_monsters_already_dealt_with() {
        let mut world = World::new();
        world.ensure_generated(-8000.0);
        let mut y = 0.0;
        let mut seen: Vec<Vec2> = Vec::new();
        for _ in 0..40 {
            let g = world
                .next_goalpost(y)
                .expect("a goalpost is always ahead on the road");
            assert!(g.y < y, "always north of the player: {g:?}");
            assert!(!seen.contains(&g.at), "{g:?} came back after it was done");
            seen.push(g.at);
            // Deal with it the way the game does, then walk on.
            for s in world.spots.iter_mut().filter(|s| s.pos == g.at) {
                s.state = SpotState::Solved;
            }
            for z in world.zones.iter_mut().filter(|z| z.pos == g.at) {
                z.cleared = true;
            }
            y = g.y - 1.0;
        }
    }

    #[test]
    fn a_blocker_goalpost_is_reached_at_the_line_it_holds() {
        let mut world = World::new();
        world.ensure_generated(-8000.0);
        let z = world.zones.iter().find(|z| z.blocks).unwrap();
        let g = goalposts_in_chunk(chunk_of(z.pos.y))
            .into_iter()
            .find(|g| g.at == z.pos)
            .unwrap();
        // The player's feet stop BLOCK_GAP south of it, so that is "0 m".
        assert_eq!(g.y, z.pos.y + BLOCK_GAP);
    }
}
