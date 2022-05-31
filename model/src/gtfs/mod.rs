mod routes;
mod shapes;
mod stop_times;
mod stops;
mod trips;

use std::collections::BTreeMap;

use anyhow::Result;
use geom::GPSBounds;
use serde::{Deserialize, Serialize};

pub use routes::{Route, RouteID};
pub use shapes::ShapeID;
pub use stop_times::StopTime;
pub use stops::{Stop, StopID};
pub use trips::{Trip, TripID};

#[derive(Serialize, Deserialize)]
pub struct GTFS {
    pub stops: BTreeMap<StopID, Stop>,
    pub routes: BTreeMap<RouteID, Route>,
    pub trips: BTreeMap<TripID, Trip>,
    pub stop_times: BTreeMap<TripID, Vec<StopTime>>,
}

impl GTFS {
    pub fn load_from_dir(gps_bounds: &GPSBounds, path: &str) -> Result<Self> {
        Ok(Self {
            stops: stops::load(gps_bounds, format!("{path}/stops.txt"))?,
            routes: routes::load(format!("{path}/routes.txt"))?,
            trips: trips::load(format!("{path}/trips.txt"))?,
            stop_times: stop_times::load(format!("{path}/stop_times.txt"))?,
        })
    }

    pub fn empty() -> Self {
        Self {
            stops: BTreeMap::new(),
            routes: BTreeMap::new(),
            trips: BTreeMap::new(),
            stop_times: BTreeMap::new(),
        }
    }
}

// TODO are routes one or both directions? (probably both)
// TODO is block_id a (useful) hint of the vehicle mapping?

// TODO next steps:
// - assign cheap numeric IDs to everything (or at least the things in World)
// - shapes: ID -> polyline
//
// then group "route patterns" or "trip groups"
// - per route, group by [stop IDs]
// - check if those always share a shape?
// - in practice, how many patterns per route? just directional and express/local
