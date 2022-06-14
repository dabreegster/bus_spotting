use std::collections::BTreeMap;

use abstutil::Timer;
use anyhow::Result;
use geom::{Distance, Time};

use crate::gtfs::{DateFilter, RouteVariant, RouteVariantID};
use crate::{Model, VehicleName};

impl Model {
    // TODO Not sure what this should fill out yet.
    pub fn segment(&self, timer: &mut Timer) -> Result<()> {
        // We're assuming the model only represents one day right now

        timer.start("match vehicles to route_short_name");

        let mut vehicles: BTreeMap<VehicleName, Assignment> = BTreeMap::new();
        for journey in &self.journeys {
            for leg in &journey.legs {
                // Somebody boarded a particular vehicle at some time, and the record claims that
                // vehicle was serving a particular route
                vehicles
                    .entry(leg.vehicle_name.clone())
                    .or_insert_with(Assignment::new)
                    .update(leg.time, &leg.route_short_name);
            }
        }

        for assignment in vehicles.values_mut() {
            assignment.segments.sort_by_key(|(t1, _, _)| *t1);
        }

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

        timer.stop("match vehicles to route_short_name");

        timer.start_iter("match vehicles to route variants", vehicles.len());
        let services = self
            .gtfs
            .calendar
            .services_matching_dates(&DateFilter::SingleDay(self.main_date));
        for (vehicle, assignment) in &vehicles {
            timer.next();

            // TODO Disable this analysis, it's slow and wrong
            continue;

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
                        variants.push(variant);
                    }
                }
            }

            println!("{:?} has {} possible variants", vehicle, variants.len());
            for variant in variants {
                if let Ok(possible_match) = PossibleMatch::new(
                    self,
                    vehicle.clone(),
                    variant,
                    assignment.segments[0].0,
                    assignment.segments[0].1,
                ) {
                    println!(
                        "  - direction changes for {:?}: {}",
                        variant.variant_id,
                        possible_match.score()
                    );
                    // TODO Total mess
                    if false {
                        for (_, dist) in possible_match.boardings {
                            println!("  - {dist}");
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

struct Assignment {
    // (time1, time2, route short name)
    // Ordered by time1
    segments: Vec<(Time, Time, String)>,
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

struct PossibleMatch {
    vehicle: VehicleName,
    variant: RouteVariantID,
    // For each boarding event, find the closest point on the variant's shape and get the distance
    // along of that. Sorted by time.
    boardings: Vec<(Time, Distance)>,
}

impl PossibleMatch {
    fn new(
        model: &Model,
        vehicle: VehicleName,
        variant: &RouteVariant,
        t1: Time,
        t2: Time,
    ) -> Result<Self> {
        let route_pl = variant.polyline(&model.gtfs)?;
        let route_short_name = model.gtfs.routes[&variant.route_id]
            .short_name
            .clone()
            .unwrap();
        let mut boardings = Vec::new();

        for journey in &model.journeys {
            for leg in &journey.legs {
                if leg.route_short_name == route_short_name && leg.time >= t1 && leg.time <= t2 {
                    if let Some((dist, _)) =
                        route_pl.dist_along_of_point(route_pl.project_pt(leg.pos))
                    {
                        boardings.push((leg.time, dist));
                    }
                }
            }
        }

        boardings.sort_by_key(|(t, _)| *t);
        Ok(Self {
            vehicle,
            variant: variant.variant_id,
            boardings,
        })
    }

    fn score(&self) -> usize {
        // You would expect boarding events over time to get snapped to increasing distance along
        // the polyline. Let's start even simpler and just count the number of times we "switch
        // directions"
        let mut dir_changes = 0;
        let mut increasing = true;
        for pair in self.boardings.windows(2) {
            let increasing_now = pair[0].1 <= pair[1].1;
            if increasing != increasing_now {
                dir_changes += 1;
                increasing = increasing_now;
            }
        }
        dir_changes
    }
}
