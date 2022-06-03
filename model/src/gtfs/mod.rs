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
}

impl GTFS {
    pub fn load_from_dir(gps_bounds: &GPSBounds, path: &str) -> Result<Self> {
        let mut gtfs = Self {
            stops: stops::load(gps_bounds, format!("{path}/stops.txt"))?,
            routes: routes::load(format!("{path}/routes.txt"))?,
        };

        let trips = trips::load(format!("{path}/trips.txt"))?;
        let mut stop_times = stop_times::load(format!("{path}/stop_times.txt"))?;

        for (trip_id, mut trip) in trips {
            trip.stop_times = match stop_times.remove(&trip_id) {
                Some(list) => list,
                None => bail!("Trip {trip_id:?} has no stop times"),
            };
            gtfs.routes
                .get_mut(&trip.route_id)
                .unwrap()
                .trips
                .insert(trip_id, trip);
        }

        if !stop_times.is_empty() {
            warn!(
                "Stop times defined for unknown trips: {:?}",
                stop_times.keys()
            );
        }

        Ok(gtfs)
    }

    pub fn empty() -> Self {
        Self {
            stops: BTreeMap::new(),
            routes: BTreeMap::new(),
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
