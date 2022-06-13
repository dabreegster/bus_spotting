mod calendar;
mod routes;
mod shapes;
mod stop_times;
mod stops;
mod trips;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use geom::GPSBounds;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

pub use calendar::{Calendar, DateFilter, DaysOfWeek, Service, ServiceID};
pub use routes::{Route, RouteID, RouteVariant, RouteVariantID};
pub use shapes::ShapeID;
pub use stop_times::StopTime;
pub use stops::{Stop, StopID};
pub use trips::{Trip, TripID};

#[derive(Serialize, Deserialize)]
pub struct GTFS {
    pub stops: BTreeMap<StopID, Stop>,
    pub routes: BTreeMap<RouteID, Route>,
    pub calendar: Calendar,
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

        let mut id_counter = 0;
        for route in gtfs.routes.values_mut() {
            group_variants(&mut id_counter, route);
        }

        gtfs.calendar = calendar::load(archive.by_name("gtfs/calendar.txt")?)?;
        calendar::load_exceptions(
            &mut gtfs.calendar,
            archive.by_name("gtfs/calendar_dates.txt")?,
        )?;

        Ok((gtfs, gps_bounds))
    }

    pub fn empty() -> Self {
        Self {
            stops: BTreeMap::new(),
            routes: BTreeMap::new(),
            calendar: Calendar {
                services: BTreeMap::new(),
            },
        }
    }

    pub fn variants_matching_dates(&self, filter: &DateFilter) -> BTreeSet<RouteVariantID> {
        let services = self.calendar.services_matching_dates(filter);
        let mut variants = BTreeSet::new();
        for route in self.routes.values() {
            for variant in &route.variants {
                if services.contains(&variant.service_id) {
                    variants.insert(variant.variant_id);
                }
            }
        }
        variants
    }

    pub fn variant(&self, id: RouteVariantID) -> &RouteVariant {
        // TODO If the ID encodes the route, we can be much better
        for route in self.routes.values() {
            for variant in &route.variants {
                if variant.variant_id == id {
                    return variant;
                }
            }
        }
        panic!("Unknown {:?}", id);
    }

    pub fn all_variants(&self) -> Vec<RouteVariantID> {
        self.routes
            .values()
            .flat_map(|route| route.variants.iter().map(|v| v.variant_id))
            .collect()
    }

    pub fn variants_for_stop(&self, stop: &StopID) -> BTreeSet<RouteVariantID> {
        // TODO Don't worry about efficiency or even readability yet.
        let mut result = BTreeSet::new();
        for route in self.routes.values() {
            for variant in &route.variants {
                if variant.stops(self).contains(stop) {
                    result.insert(variant.variant_id);
                }
            }
        }
        result
    }
}

// TODO is block_id a (useful) hint of the vehicle mapping?

// TODO next steps:
// - assign cheap numeric IDs to everything (or at least the things in World)
// - shapes: ID -> polyline

fn group_variants(id_counter: &mut usize, route: &mut Route) {
    // TODO Also group by shape ID, outbound direction?
    // in practice, how many patterns per route? just directional and express/local?
    //
    // (Stops, headsign, service)
    type Key = (Vec<StopID>, Option<String>, ServiceID);

    let mut variants: BTreeMap<Key, Vec<TripID>> = BTreeMap::new();
    for trip in route.trips.values() {
        let stops: Vec<StopID> = trip
            .stop_times
            .iter()
            .map(|st| st.stop_id.clone())
            .collect();
        let key = (stops, trip.headsign.clone(), trip.service_id.clone());
        variants
            .entry(key)
            .or_insert_with(Vec::new)
            .push(trip.trip_id.clone());
    }

    for ((_, headsign, service_id), mut trips) in variants {
        trips.sort_by_key(|t| route.trips[t].stop_times[0].arrival_time);

        route.variants.push(RouteVariant {
            route_id: route.route_id.clone(),
            variant_id: RouteVariantID(*id_counter),
            trips,
            headsign,
            service_id,
        });
        *id_counter += 1;
    }
}
