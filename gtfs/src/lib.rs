#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod calendar;
mod ids;
mod routes;
mod shapes;
mod snap;
mod stop_times;
mod stops;
mod trips;

use std::collections::{BTreeMap, BTreeSet};

use abstutil::Timer;
use anyhow::Result;
use geom::{GPSBounds, PolyLine, Polygon};
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

pub use calendar::{Calendar, DateFilter, DaysOfWeek, Service, ServiceID};
pub use ids::{orig, CheapID, IDMapping, StopID, TripID};
pub use routes::{Route, RouteID, RouteVariant, RouteVariantID};
pub use shapes::ShapeID;
pub use stop_times::StopTime;
pub use stops::Stop;
pub use trips::Trip;

#[derive(Clone, Serialize, Deserialize)]
pub struct GTFS {
    pub stops: BTreeMap<StopID, Stop>,
    pub routes: BTreeMap<RouteID, Route>,
    pub calendar: Calendar,
    pub shapes: BTreeMap<ShapeID, PolyLine>,
    // Some shapes optionally snapped to a street network
    pub snapped_shapes: BTreeMap<ShapeID, PolyLine>,

    // This is only retained for debugging / visualization. Once StreetNetwork stashes final
    // geometry directly, this could be simpler.
    pub road_geometry: Vec<Polygon>,
    pub intersection_geometry: Vec<Polygon>,
}

impl GTFS {
    pub fn load_from_dir(
        archive: &mut ZipArchive<std::io::Cursor<Vec<u8>>>,
        timer: &mut Timer,
    ) -> Result<(Self, GPSBounds)> {
        let mut gtfs = Self::empty();
        let (stops, stop_ids, gps_bounds) = stops::load(archive.by_name("gtfs/stops.txt")?)?;
        gtfs.stops = stops;
        gtfs.routes = routes::load(archive.by_name("gtfs/routes.txt")?)?;
        if let Ok(file) = archive.by_name("gtfs/shapes.txt") {
            gtfs.shapes = shapes::load(file, &gps_bounds)?;
        }

        let (trips, trip_ids) = trips::load(archive.by_name("gtfs/trips.txt")?)?;
        let mut stop_times = stop_times::load(
            archive.by_name("gtfs/stop_times.txt")?,
            &stop_ids,
            &trip_ids,
        )?;

        let mut trips_per_route: BTreeMap<RouteID, Vec<Trip>> = BTreeMap::new();
        for mut trip in trips {
            trip.stop_times = match stop_times.remove(&trip.id) {
                Some(list) => list,
                None => bail!("Trip {:?} has no stop times", trip.orig_id),
            };
            trips_per_route
                .entry(trip.route_id.clone())
                .or_insert_with(Vec::new)
                .push(trip);
        }

        if !stop_times.is_empty() {
            warn!(
                "Stop times defined for unknown trips: {:?}",
                stop_times.keys()
            );
        }

        let mut id_counter = 0;
        for route in gtfs.routes.values_mut() {
            group_variants(
                &mut id_counter,
                route,
                trips_per_route.remove(&route.route_id).unwrap(),
            );
        }

        // Find all variants per stop
        for route in gtfs.routes.values() {
            for variant in &route.variants {
                for stop in variant.stops() {
                    gtfs.stops
                        .get_mut(&stop)
                        .unwrap()
                        .route_variants
                        .insert(variant.variant_id);
                }
            }
        }

        gtfs.calendar = calendar::load(archive.by_name("gtfs/calendar.txt")?)?;
        calendar::load_exceptions(
            &mut gtfs.calendar,
            archive.by_name("gtfs/calendar_dates.txt")?,
        )?;

        if let Ok(osm_xml_input) = archive.by_name("osm_input.xml") {
            snap::snap_routes(&mut gtfs, osm_xml_input, &gps_bounds, timer)?;
        }

        dump_bounding_box(&gps_bounds);

        Ok((gtfs, gps_bounds))
    }

    pub fn empty() -> Self {
        Self {
            stops: BTreeMap::new(),
            routes: BTreeMap::new(),
            calendar: Calendar {
                services: BTreeMap::new(),
            },
            shapes: BTreeMap::new(),
            snapped_shapes: BTreeMap::new(),
            road_geometry: Vec::new(),
            intersection_geometry: Vec::new(),
        }
    }

    pub fn variants_matching_filter(&self, filter: &VariantFilter) -> BTreeSet<RouteVariantID> {
        let services = self.calendar.services_matching_dates(&filter.date_filter);
        let mut variants = BTreeSet::new();
        for route in self.routes.values() {
            for variant in &route.variants {
                // TODO I think this is correct, but make sure trips per variant is daily
                if services.contains(&variant.service_id)
                    && variant.trips.len() >= filter.minimum_trips_per_day
                {
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

    pub fn parent_of_variant(&self, id: RouteVariantID) -> &Route {
        // TODO If the ID encodes the route, we can be much better
        for route in self.routes.values() {
            for variant in &route.variants {
                if variant.variant_id == id {
                    return route;
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
}

fn group_variants(id_counter: &mut usize, route: &mut Route, trips: Vec<Trip>) {
    // (Stops, headsign, service, shape)
    type Key = (Vec<StopID>, Option<String>, ServiceID, ShapeID);

    let mut variants: BTreeMap<Key, Vec<Trip>> = BTreeMap::new();
    for trip in trips {
        let stops: Vec<StopID> = trip.stop_times.iter().map(|st| st.stop_id).collect();
        let key = (
            stops,
            trip.headsign.clone(),
            trip.service_id.clone(),
            trip.shape_id.clone(),
        );
        variants.entry(key).or_insert_with(Vec::new).push(trip);
    }

    for ((_, headsign, service_id, shape_id), mut trips) in variants {
        trips.sort_by_key(|t| t.stop_times[0].arrival_time);

        route.variants.push(RouteVariant {
            route_id: route.route_id.clone(),
            variant_id: RouteVariantID(*id_counter),
            trips,
            headsign,
            service_id,
            shape_id,
        });
        *id_counter += 1;
    }
}

/// Filter for route variants
pub struct VariantFilter {
    pub date_filter: DateFilter,
    pub minimum_trips_per_day: usize,
}

fn dump_bounding_box(gps_bounds: &GPSBounds) {
    use geojson::{Feature, FeatureCollection, GeoJson};

    let feature = Feature {
        bbox: None,
        geometry: Some(
            gps_bounds
                .to_bounds()
                .get_rectangle()
                .to_geojson(Some(gps_bounds)),
        ),
        id: None,
        properties: None,
        foreign_members: None,
    };
    let gj = GeoJson::FeatureCollection(FeatureCollection {
        features: vec![feature],
        bbox: None,
        foreign_members: None,
    });
    info!(
        "GeoJSON covering the bounding box: {}",
        serde_json::to_string(&gj).unwrap()
    );
}
