mod routes;
mod shapes;
mod stop_times;
mod stops;
mod trips;

use std::collections::BTreeMap;

use anyhow::Result;
use geom::GPSBounds;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

pub use routes::{Route, RouteID, RouteVariant, RouteVariantID};
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
    pub fn load_from_dir(
        archive: &mut ZipArchive<std::io::Cursor<Vec<u8>>>,
    ) -> Result<(Self, GPSBounds)> {
        let mut gtfs = Self::empty();
        let (stops, gps_bounds) = stops::load(archive.by_name("gtfs/stops.txt")?)?;
        gtfs.stops = stops;
        gtfs.routes = routes::load(archive.by_name("gtfs/routes.txt")?)?;

        let trips = trips::load(archive.by_name("gtfs/trips.txt")?)?;
        let mut stop_times = stop_times::load(archive.by_name("gtfs/stop_times.txt")?)?;

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

        for route in gtfs.routes.values_mut() {
            group_variants(route);
        }

        Ok((gtfs, gps_bounds))
    }

    pub fn empty() -> Self {
        Self {
            stops: BTreeMap::new(),
            routes: BTreeMap::new(),
        }
    }
}

// TODO is block_id a (useful) hint of the vehicle mapping?

// TODO next steps:
// - assign cheap numeric IDs to everything (or at least the things in World)
// - shapes: ID -> polyline

fn group_variants(route: &mut Route) {
    // TODO Also group by shape ID, outbound direction?
    // in practice, how many patterns per route? just directional and express/local?
    type Key = (Vec<StopID>, Option<String>);

    let mut variants: BTreeMap<Key, Vec<TripID>> = BTreeMap::new();
    for trip in route.trips.values() {
        let stops: Vec<StopID> = trip
            .stop_times
            .iter()
            .map(|st| st.stop_id.clone())
            .collect();
        let key = (stops, trip.headsign.clone());
        variants
            .entry(key)
            .or_insert_with(Vec::new)
            .push(trip.trip_id.clone());
    }

    for ((_, headsign), trips) in variants {
        route.variants.push(RouteVariant {
            route_id: route.route_id.clone(),
            variant_id: RouteVariantID(route.variants.len()),
            trips,
            headsign,
        });
    }
}
