//! The discovery map (FR27): how far up the road the player has been, the
//! landmarks and regions that lie behind that point, and the castle that waits
//! at the far end, the one thing the fog never hides.
//!
//! The map is not stored. The world is a pure function of `y` and the chunk
//! index, so the map samples it: it only has to remember the furthest point
//! reached. Everything north of that is fog. This module knows nothing about
//! egui; `ui::map_panel` draws what `Discovery::snapshot` returns.

use super::monster::MonsterKind;
use super::regions;
use super::world::{self, SpotState, World};

/// The map always shows at least this much road, so a fresh run isn't a stub.
pub const MIN_RANGE_M: f32 = 150.0;
/// Fog shown above the furthest point reached, so the edge reads as fog.
pub const FOG_AHEAD_M: f32 = 40.0;
/// Spacing of the distance markers on the map.
pub const MILESTONE_M: u32 = 50;

/// What a map icon stands for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LandmarkKind {
    /// The iron gate where the road starts.
    Gate,
    /// A stone milestone every `MILESTONE_M`.
    Milestone,
    /// A potion flask not yet opened.
    FlaskFresh,
    FlaskSolved,
    FlaskMissed,
    /// A monster zone not yet beaten.
    Monster,
    MonsterBeaten,
    /// A Tide Titan's zone, beaten or not.
    Boss {
        beaten: bool,
    },
    /// A safe stop (FR28).
    Stop,
    /// The castle at the end of the road: always on the map, in the fog or not.
    Castle,
}

#[derive(Clone, Copy, Debug)]
pub struct Landmark {
    pub kind: LandmarkKind,
    /// Metres up the road from the gate.
    pub meters: f32,
    /// The road's centre at that point, in world pixels, relative to the
    /// gate's centre: the panel scales it to map pixels.
    pub dx: f32,
    pub label: Option<u32>,
}

/// One region the player has entered, as a band along the road (FR27).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionBand {
    pub name: &'static str,
    /// Metres up the road where the band starts and where it stops: the
    /// region's end, or the furthest point reached if it hasn't been crossed.
    pub from_m: f32,
    pub to_m: f32,
    pub color: [u8; 3],
}

/// Everything the panel needs to draw one frame of the map.
#[derive(Clone, Debug)]
pub struct MapView {
    /// Metres shown from the gate to the top of the map.
    pub range_m: f32,
    pub furthest_m: f32,
    pub player_m: f32,
    pub player_dx: f32,
    pub landmarks: Vec<Landmark>,
    /// The road's centre every `step_m`, for the discovered stretch only.
    pub road: Vec<(f32, f32)>,
    /// The regions entered so far, south to north. Regions further up are not
    /// listed: their names stay in the fog.
    pub regions: Vec<RegionBand>,
    /// Metres from the gate to the castle.
    pub castle_m: f32,
    /// Metres left for the player to walk to the castle gate.
    pub to_go_m: f32,
}

/// How far the player has got. Resets with the run.
#[derive(Clone, Copy, Debug)]
pub struct Discovery {
    /// The northmost (smallest) world `y` reached.
    furthest_y: f32,
}

impl Default for Discovery {
    fn default() -> Self {
        Self::new()
    }
}

fn meters_at(y: f32) -> f32 {
    ((world::ROAD_START_Y - y) / world::PX_PER_METER).max(0.0)
}

fn y_at(meters: f32) -> f32 {
    world::ROAD_START_Y - meters * world::PX_PER_METER
}

fn dx_at(y: f32) -> f32 {
    world::path_center(y) - world::path_center(world::ROAD_START_Y)
}

impl Discovery {
    pub fn new() -> Self {
        Self {
            furthest_y: world::ROAD_START_Y,
        }
    }

    /// Records that the player stood at `y`.
    pub fn note(&mut self, y: f32) {
        self.furthest_y = self.furthest_y.min(y);
    }

    pub fn furthest_m(&self) -> f32 {
        meters_at(self.furthest_y)
    }

    /// True when a point `y` on the road lies in the discovered part.
    pub fn is_discovered(&self, y: f32) -> bool {
        y >= self.furthest_y
    }

    /// Builds what the panel draws. `player_y` is where the player stands now.
    pub fn snapshot(&self, world: &World, player_y: f32, player_x: f32) -> MapView {
        let furthest_m = self.furthest_m();
        let range_m = (furthest_m + FOG_AHEAD_M).max(MIN_RANGE_M);

        let mut landmarks = vec![Landmark {
            kind: LandmarkKind::Gate,
            meters: 0.0,
            dx: 0.0,
            label: None,
        }];
        landmarks.push(Landmark {
            kind: LandmarkKind::Castle,
            meters: world::CASTLE_M,
            dx: dx_at(world::CASTLE_GATE_Y),
            label: None,
        });
        let mut m = MILESTONE_M;
        while (m as f32) <= furthest_m && (m as f32) < world::CASTLE_M {
            landmarks.push(Landmark {
                kind: LandmarkKind::Milestone,
                meters: m as f32,
                dx: dx_at(y_at(m as f32)),
                label: Some(m),
            });
            m += MILESTONE_M;
        }
        for s in &world.spots {
            if !self.is_discovered(s.pos.y) || s.pos.y > world::ROAD_START_Y {
                continue;
            }
            let kind = match s.state {
                SpotState::Fresh => LandmarkKind::FlaskFresh,
                SpotState::Pending => continue,
                SpotState::Solved => LandmarkKind::FlaskSolved,
                SpotState::Missed => LandmarkKind::FlaskMissed,
            };
            landmarks.push(Landmark {
                kind,
                meters: meters_at(s.pos.y),
                dx: dx_at(s.pos.y),
                label: None,
            });
        }
        for s in &world.stops {
            if !self.is_discovered(s.y) {
                continue;
            }
            landmarks.push(Landmark {
                kind: LandmarkKind::Stop,
                meters: meters_at(s.y),
                dx: dx_at(s.y),
                label: None,
            });
        }
        for z in &world.zones {
            if !self.is_discovered(z.pos.y) {
                continue;
            }
            let kind = if matches!(z.kind, MonsterKind::TideTitan) {
                LandmarkKind::Boss { beaten: z.cleared }
            } else if z.cleared {
                LandmarkKind::MonsterBeaten
            } else {
                LandmarkKind::Monster
            };
            landmarks.push(Landmark {
                kind,
                meters: meters_at(z.pos.y),
                dx: dx_at(z.pos.y),
                label: None,
            });
        }

        let step = 5.0;
        let mut road = Vec::new();
        let mut at = 0.0;
        while at < furthest_m {
            road.push((at, dx_at(y_at(at))));
            at += step;
        }
        road.push((furthest_m, dx_at(self.furthest_y)));

        let regions = regions::REGIONS
            .iter()
            .enumerate()
            .map(|(i, r)| RegionBand {
                name: r.name,
                from_m: r.from_m as f32,
                to_m: regions::end_m(i, world::CASTLE_M).min(furthest_m),
                color: r.color,
            })
            .filter(|b| b.to_m > b.from_m)
            .collect();
        let player_m = meters_at(player_y);

        MapView {
            range_m,
            furthest_m,
            player_m,
            player_dx: player_x - world::path_center(world::ROAD_START_Y),
            landmarks,
            road,
            regions,
            castle_m: world::CASTLE_M,
            to_go_m: (world::CASTLE_M - player_m).max(0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_the_gate() {
        let d = Discovery::new();
        assert_eq!(d.furthest_m(), 0.0);
    }

    #[test]
    fn only_ever_moves_north() {
        let mut d = Discovery::new();
        d.note(y_at(80.0));
        d.note(y_at(30.0)); // walked back
        d.note(world::PLAYER_START_Y); // in the hub
        assert!((d.furthest_m() - 80.0).abs() < 0.01);
    }

    #[test]
    fn range_never_drops_below_the_minimum_and_grows_with_progress() {
        let mut d = Discovery::new();
        let w = World::new();
        assert_eq!(
            d.snapshot(&w, world::PLAYER_START_Y, 0.0).range_m,
            MIN_RANGE_M
        );
        d.note(y_at(400.0));
        assert_eq!(
            d.snapshot(&w, y_at(400.0), 0.0).range_m,
            400.0 + FOG_AHEAD_M
        );
    }

    #[test]
    fn landmarks_stay_inside_the_discovered_road() {
        let mut w = World::new();
        w.ensure_generated(y_at(600.0));
        let mut d = Discovery::new();
        d.note(y_at(300.0));
        let v = d.snapshot(&w, y_at(300.0), 0.0);
        for l in v
            .landmarks
            .iter()
            .filter(|l| l.kind != LandmarkKind::Castle)
        {
            assert!(l.meters <= v.furthest_m + 0.01, "{l:?} is in the fog");
        }
        // Flasks and monsters further up exist but stay hidden.
        assert!(w.spots.iter().any(|s| meters_at(s.pos.y) > 300.0));
        assert!(v
            .landmarks
            .iter()
            .any(|l| l.kind == LandmarkKind::FlaskFresh));
    }

    #[test]
    fn milestones_every_fifty_metres() {
        let mut d = Discovery::new();
        d.note(y_at(120.0));
        let v = d.snapshot(&World::new(), y_at(120.0), 0.0);
        let ms: Vec<_> = v
            .landmarks
            .iter()
            .filter(|l| l.kind == LandmarkKind::Milestone)
            .filter_map(|l| l.label)
            .collect();
        assert_eq!(ms, vec![50, 100]);
    }

    #[test]
    fn the_same_walk_gives_the_same_map() {
        let make = || {
            let mut w = World::new();
            w.ensure_generated(y_at(200.0));
            let mut d = Discovery::new();
            d.note(y_at(180.0));
            d.snapshot(&w, y_at(180.0), 0.0).landmarks.len()
        };
        assert_eq!(make(), make());
    }

    #[test]
    fn solved_flasks_show_as_solved() {
        let mut w = World::new();
        w.ensure_generated(y_at(100.0));
        let i = w
            .spots
            .iter()
            .position(|s| s.pos.y <= world::ROAD_START_Y)
            .unwrap();
        let y = w.spots[i].pos.y;
        w.spots[i].state = SpotState::Solved;
        let mut d = Discovery::new();
        d.note(y - 10.0);
        let v = d.snapshot(&w, y, 0.0);
        assert!(v
            .landmarks
            .iter()
            .any(|l| l.kind == LandmarkKind::FlaskSolved));
    }

    #[test]
    fn the_castle_is_on_the_map_from_the_first_step() {
        let v = Discovery::new().snapshot(&World::new(), world::PLAYER_START_Y, 0.0);
        let castle = v
            .landmarks
            .iter()
            .find(|l| l.kind == LandmarkKind::Castle)
            .expect("the castle shows in the fog");
        assert_eq!(castle.meters, world::CASTLE_M);
        assert_eq!(v.castle_m, world::CASTLE_M);
        assert_eq!(v.to_go_m, world::CASTLE_M);
    }

    #[test]
    fn the_distance_to_go_counts_down() {
        let mut d = Discovery::new();
        d.note(y_at(300.0));
        let v = d.snapshot(&World::new(), y_at(300.0), 0.0);
        assert!((v.to_go_m - (world::CASTLE_M - 300.0)).abs() < 0.1);
        // At the gate there's nothing left.
        d.note(world::CASTLE_GATE_Y);
        let v = d.snapshot(&World::new(), world::CASTLE_GATE_Y, 0.0);
        assert_eq!(v.to_go_m, 0.0);
    }

    #[test]
    fn only_the_regions_entered_have_a_band() {
        let mut d = Discovery::new();
        assert!(d
            .snapshot(&World::new(), world::PLAYER_START_Y, 0.0)
            .regions
            .is_empty());

        d.note(y_at(300.0));
        let v = d.snapshot(&World::new(), y_at(300.0), 0.0);
        assert_eq!(v.regions.len(), 1);
        assert_eq!(v.regions[0].name, "The Dark Forest");
        assert_eq!(
            v.regions[0].to_m, 300.0,
            "an open band ends at the furthest point"
        );

        // Past the first border: the forest is closed off at its own end, and
        // the next region's name appears. Names further up stay hidden.
        d.note(y_at(950.0));
        let v = d.snapshot(&World::new(), y_at(950.0), 0.0);
        let names: Vec<_> = v.regions.iter().map(|b| b.name).collect();
        assert_eq!(names, ["The Dark Forest", "Tallybrook Fields"]);
        assert_eq!(v.regions[0].to_m, 900.0);
        assert_eq!(v.regions[1].to_m, 950.0);
    }

    #[test]
    fn the_whole_road_is_covered_by_bands_at_the_gate() {
        let mut d = Discovery::new();
        d.note(world::CASTLE_GATE_Y);
        let v = d.snapshot(&World::new(), world::CASTLE_GATE_Y, 0.0);
        assert_eq!(v.regions.len(), regions::REGIONS.len());
        for w in v.regions.windows(2) {
            assert_eq!(w[0].to_m, w[1].from_m, "no gap between regions");
        }
        assert_eq!(v.regions.last().unwrap().to_m, world::CASTLE_M);
    }
}
