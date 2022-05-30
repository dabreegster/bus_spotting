use std::collections::BTreeMap;

mod stops;

use anyhow::Result;
use geom::GPSBounds;

pub use stops::{Stop, StopID};

pub struct GTFS {
    pub stops: BTreeMap<StopID, Stop>,
}

impl GTFS {
    pub fn load_from_dir(gps_bounds: &GPSBounds, path: &str) -> Result<Self> {
        Ok(Self {
            stops: stops::load(gps_bounds, format!("{path}/stops.txt"))?,
        })
    }
}

// TODO are routes one or both directions? (probably both)
// TODO is block_id a (useful) hint of the vehicle mapping?

// TODO next steps:
// - assign cheap numeric IDs to everything (or at least the things in World)
// - routes: ID -> metadata
// - trips: ID -> route, shape, direction
// - stop_times: trip -> [stop ID, arrival time, departure time]
// - shapes: ID -> polyline
//
// then group "route patterns" or "trip groups"
// - per route, group by [stop IDs]
// - check if those always share a shape?
// - in practice, how many patterns per route? just directional and express/local
