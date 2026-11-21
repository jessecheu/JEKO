//! The discovery map (FR27, milestone A): how far up the road the player has
//! been, and the landmarks that lie behind that point.
//!
//! The map is not stored. The world is a pure function of `y` and the chunk
//! index, so the map samples it: it only has to remember the furthest point
//! reached. Everything north of that is fog. This module knows nothing about
//! egui; `ui::map_panel` draws what `Discovery::snapshot` returns.

use super::monster::MonsterKind;
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
        let mut m = MILESTONE_M;
        while (m as f32) <= furthest_m {
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

        MapView {
            range_m,
            furthest_m,
            player_m: meters_at(player_y),
            player_dx: player_x - world::path_center(world::ROAD_START_Y),
            landmarks,
            road,
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
        for l in &v.landmarks {
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
}
