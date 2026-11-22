//! The regions of the road (FR27, milestone B): the dark forest, the fields
//! outside Tallybrook, the village itself and the castle grounds, laid end to
//! end by distance from the gate.
//!
//! The table is the single place a region is defined. Adding one is one entry
//! in [`REGIONS`]: the banner, the map band and label, the border sign, the
//! goalpost and the scenery all read it. Nothing here knows about macroquad or
//! egui (invariant 10); `decor::generate_chunk` turns a region's [`Scenery`]
//! into props, and `ui::map_panel` turns its colour into a band.

/// How a region changes the random scenery `decor::generate_chunk` scatters.
/// Every knob is a share of what the dark forest would have there, so the
/// forest itself (all `1.0` and `0.0`) is exactly the scenery the road has
/// always had.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scenery {
    /// Share of the woods' pines kept (1 = all of them).
    pub pines: f32,
    /// Share of the old graveyard's stones, pillars and scarecrows kept.
    pub graves: f32,
    /// Chance per chunk of an extra cottage beside the road.
    pub cottages: f32,
    /// Chance per chunk of a haystack on the verge.
    pub hay: f32,
    /// Chance per chunk of a well beside the road.
    pub wells: f32,
    /// Chance per chunk of an extra lamppost beside the road.
    pub lamps: f32,
    /// Chance per chunk of a stone column by the road, for the grounds before
    /// the castle.
    pub pillars: f32,
}

/// One stretch of the road.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    /// The name shown on the banner, the border sign, the map and the tracker.
    pub name: &'static str,
    /// Metres up the road from the gate where it starts. The first region
    /// starts at 0; each runs until the next one starts (the last, until the
    /// castle).
    pub from_m: u32,
    /// The map's colour for the region's band, as sRGB.
    pub color: [u8; 3],
    pub scenery: Scenery,
}

const FOREST: Scenery = Scenery {
    pines: 1.0,
    graves: 1.0,
    cottages: 0.0,
    hay: 0.0,
    wells: 0.0,
    lamps: 0.0,
    pillars: 0.0,
};

/// The regions, south to north. `from_m` must rise, the first must be 0, and
/// every `from_m` must sit short of `world::CASTLE_M`; the tests check it.
pub const REGIONS: [Region; 4] = [
    Region {
        name: "The Dark Forest",
        from_m: 0,
        color: [86, 112, 84],
        scenery: FOREST,
    },
    Region {
        name: "Tallybrook Fields",
        from_m: 900,
        color: [168, 176, 80],
        scenery: Scenery {
            pines: 0.55,
            graves: 0.5,
            cottages: 0.25,
            hay: 0.7,
            wells: 0.0,
            lamps: 0.15,
            pillars: 0.0,
        },
    },
    Region {
        name: "Tallybrook Village",
        from_m: 1100,
        color: [214, 118, 90],
        scenery: Scenery {
            pines: 0.3,
            graves: 0.15,
            cottages: 0.85,
            hay: 0.25,
            wells: 0.3,
            lamps: 0.8,
            pillars: 0.0,
        },
    },
    Region {
        name: "The Castle Grounds",
        from_m: 1400,
        color: [124, 108, 166],
        scenery: Scenery {
            pines: 0.45,
            graves: 0.6,
            cottages: 0.0,
            hay: 0.0,
            wells: 0.0,
            lamps: 0.9,
            pillars: 0.7,
        },
    },
];

/// The index into [`REGIONS`] of the region `meters` up the road falls in.
pub fn index_at(meters: f32) -> usize {
    REGIONS
        .iter()
        .rposition(|r| meters >= r.from_m as f32)
        .unwrap_or(0)
}

/// The region `meters` up the road falls in.
pub fn at(meters: f32) -> &'static Region {
    &REGIONS[index_at(meters)]
}

/// Where region `i` ends: where the next starts, or the castle.
pub fn end_m(i: usize, castle_m: f32) -> f32 {
    REGIONS.get(i + 1).map_or(castle_m, |r| r.from_m as f32)
}

/// The region that starts exactly at `from_m`, for the tracker's label.
pub fn starting_at(from_m: u32) -> Option<&'static Region> {
    REGIONS.iter().find(|r| r.from_m == from_m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::world;

    #[test]
    fn the_table_is_ordered_and_inside_the_road() {
        assert_eq!(REGIONS[0].from_m, 0);
        for w in REGIONS.windows(2) {
            assert!(
                w[0].from_m < w[1].from_m,
                "{} before {}",
                w[0].name,
                w[1].name
            );
        }
        for r in &REGIONS {
            assert!(
                (r.from_m as f32) < world::CASTLE_M,
                "{} starts past the castle",
                r.name
            );
        }
    }

    #[test]
    fn a_distance_falls_in_exactly_one_region() {
        assert_eq!(at(0.0).name, "The Dark Forest");
        assert_eq!(at(899.9).name, "The Dark Forest");
        assert_eq!(at(900.0).name, "Tallybrook Fields");
        assert_eq!(at(1250.0).name, "Tallybrook Village");
        assert_eq!(at(world::CASTLE_M).name, "The Castle Grounds");
        // The hub is south of the gate: it reads as the first region.
        assert_eq!(index_at(-30.0), 0);
    }

    #[test]
    fn regions_end_where_the_next_begins() {
        assert_eq!(end_m(0, world::CASTLE_M), 900.0);
        assert_eq!(end_m(REGIONS.len() - 1, world::CASTLE_M), world::CASTLE_M);
    }

    #[test]
    fn the_forest_changes_nothing() {
        assert_eq!(REGIONS[0].scenery, FOREST);
    }

    #[test]
    fn every_knob_is_a_share() {
        for r in &REGIONS {
            let s = r.scenery;
            for v in [
                s.pines, s.graves, s.cottages, s.hay, s.wells, s.lamps, s.pillars,
            ] {
                assert!((0.0..=1.0).contains(&v), "{}: {v}", r.name);
            }
        }
    }

    #[test]
    fn names_are_short_enough_for_a_banner_and_a_sign() {
        for r in &REGIONS {
            assert!(r.name.len() <= 24, "{}", r.name);
        }
    }
}
