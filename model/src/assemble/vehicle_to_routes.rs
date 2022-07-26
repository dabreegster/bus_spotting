use std::collections::BTreeMap;

use anyhow::Result;
use geom::Time;

use crate::{DailyModel, VehicleID, VehicleName};
use gtfs::{DateFilter, RouteVariantID};

// BIL data says somebody boarded a vehicle and rode route_short_name. Use that, along with the
// variants actually served that day, to figure out possible route variants per vehicle.

impl DailyModel {
    // Expensive -- calculates a mapping for the full day, but only uses one vehicle
    pub fn vehicle_to_possible_routes(&self, id: VehicleID) -> Vec<RouteVariantID> {
        match self.vehicles_to_possible_routes().unwrap().remove(&id) {
            Some(list) => list,
            None => Vec::new(),
        }
    }

    pub(crate) fn vehicles_to_possible_routes(
        &self,
    ) -> Result<BTreeMap<VehicleID, Vec<RouteVariantID>>> {
        let services = self
            .gtfs
            .calendar
            .services_matching_dates(&DateFilter::SingleDay(self.date));
        let mut result = BTreeMap::new();
        for (vehicle, assignment) in self.vehicle_to_route_short_name()? {
            // Start simple
            if assignment.segments.len() != 1 {
                continue;
            }
            // What variants match?
            let mut variants = Vec::new();
            for route in self.gtfs.routes.values() {
                if route.short_name.as_ref() != Some(&assignment.segments[0].2) {
                    continue;
                }
                for variant in &route.variants {
                    if services.contains(&variant.service_id) {
                        variants.push(variant.variant_id);
                    }
                }
            }
            if !variants.is_empty() {
                result.insert(vehicle, variants);
            }
        }
        Ok(result)
    }

    pub(crate) fn vehicle_to_route_short_name(&self) -> Result<BTreeMap<VehicleID, Assignment>> {
        let mut vehicles: BTreeMap<VehicleID, Assignment> = BTreeMap::new();
        for journey in &self.journeys {
            for leg in &journey.legs {
                // Ignore when BIL refers to vehicles we don't know from AVL
                let vehicle = if let Ok(x) = self.vehicle_ids.lookup(&leg.vehicle_name) {
                    x
                } else {
                    continue;
                };

                // Somebody boarded a particular vehicle at some time, and the record claims that
                // vehicle was serving a particular route
                vehicles
                    .entry(vehicle)
                    .or_insert_with(Assignment::new)
                    .update(leg.time, &leg.route_short_name);
            }
        }

        for assignment in vehicles.values_mut() {
            assignment.segments.sort_by_key(|(t1, _, _)| *t1);
        }

        // Just print some stats
        if false {
            let mut one_route = 0;
            let mut multiple_routes_normal = 0;
            let mut multiple_routes_overlapping = 0;
            for (vehicle, assignment) in &vehicles {
                // Most serve 1 route; that's the simple case for matching
                if assignment.segments.len() == 1 {
                    one_route += 1;
                    continue;
                }

                if !assignment.has_overlaps() {
                    multiple_routes_normal += 1;
                    continue;
                }

                multiple_routes_overlapping += 1;
                println!(
                    "{:?} serves {} routes, partly overlapping",
                    vehicle,
                    assignment.segments.len()
                );
                for (t1, t2, route) in &assignment.segments {
                    println!("  - from {t1} to {t2}: {route}");
                }
            }

            println!("{one_route} vehicles serve 1 route, {multiple_routes_normal} serve multiple normally, {multiple_routes_overlapping} have weird overlaps");
        }

        // Manually debug vehicles assigned to multiple routes with apparent overlap
        // To understand best, then do `sort -n vehicle_assignment.csv`
        if false && cfg!(not(target_arch = "wasm32")) {
            use std::fs::File;
            use std::io::Write;

            let mut f = File::create("vehicle_assignment.csv")?;
            writeln!(f, "time,route_short_name")?;
            let debug = VehicleName("03279".to_string());

            for journey in &self.journeys {
                for leg in &journey.legs {
                    if leg.vehicle_name == debug {
                        writeln!(f, "{},{}", leg.time.inner_seconds(), leg.route_short_name)?;
                    }
                }
            }
        }

        Ok(vehicles)
    }
}

// TODO Move this to experiments, and simplify the above implementations, which just look for all
// possible variants and ignore time / possible overlaps entirely.

// If a vehicle serves multiple routes over the day, try to figure out when they switch.
//
// Current implementation doesn't handle a sequence like (route1, route2, route1). Could improve it
// by first sorting by time all ticketing events per vehicle, then processing those in order.
pub(crate) struct Assignment {
    // (time1, time2, route short name)
    // Ordered by time1
    pub(crate) segments: Vec<(Time, Time, String)>,
}

impl Assignment {
    fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    fn update(&mut self, time: Time, route: &str) {
        // Do we already serve this route?
        for (t1, t2, route_short_name) in &mut self.segments {
            if route != route_short_name {
                continue;
            }
            // Extend the time interval if needed
            if time < *t1 {
                *t1 = time;
            } else if time > *t2 {
                *t2 = time;
            }
            return;
        }

        // This vehicle is serving another route. Start an empty interval for it.
        self.segments.push((time, time, route.to_string()));
    }

    // Checks for unusual situations worth debugging further. Assumes segments have been sorted.
    fn has_overlaps(&self) -> bool {
        for pair in self.segments.windows(2) {
            let (_t1, t2, _) = pair[0];
            let (t3, _t4, _) = pair[1];

            if t3 < t2 {
                return true;
            }
        }
        false
    }
}
