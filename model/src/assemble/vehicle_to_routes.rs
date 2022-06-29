use std::collections::BTreeMap;

use anyhow::Result;
use geom::{Distance, Time};

use crate::gtfs::{DateFilter, RouteVariantID, TripID};
use crate::{Model, Trajectory, VehicleID, VehicleName};

impl Model {
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
            .services_matching_dates(&DateFilter::SingleDay(self.main_date));
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

pub struct Assignment {
    // (time1, time2, route short name)
    // Ordered by time1
    pub segments: Vec<(Time, Time, String)>,
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

impl Model {
    pub fn possible_trip_trajectories_for_vehicle(
        &self,
        id: VehicleID,
    ) -> Result<Vec<(String, Trajectory)>> {
        let variants = self.vehicle_to_possible_routes(id);

        let mut result = Vec::new();
        for variant in variants {
            let variant_description = self.gtfs.variant(variant).describe(&self.gtfs);
            for (trip, trajectory) in self.trajectories_for_variant(variant)? {
                result.push((format!("{:?} of {}", trip, variant_description), trajectory));
            }
        }
        Ok(result)
    }

    pub fn possible_route_trajectories_for_vehicle(
        &self,
        id: VehicleID,
    ) -> Result<Vec<(String, Trajectory)>> {
        let variants = self.vehicle_to_possible_routes(id);

        // We could group by shape, but the UI actually cares about disambiguating, so don't bother
        let mut result = Vec::new();
        for variant in variants {
            // This doesn't take time into account at all
            let variant = self.gtfs.variant(variant);
            let pl = &self.gtfs.shapes[&variant.shape_id];
            result.push((variant.describe(&self.gtfs), Trajectory::from_polyline(pl)));
        }
        Ok(result)
    }

    pub fn look_for_explainable_vehicle(&self) {
        let mut best = Vec::new();
        for vehicle in &self.vehicles {
            let mut scores = self.score_vehicle_similarity_to_trips(vehicle.id);
            if !scores.is_empty() {
                let (trip, score) = scores.remove(0);
                best.push((vehicle.id, trip, score));
            }
        }
        best.sort_by_key(|pair| pair.2);
        println!("Any good matches?");
        for (vehicle, trip, score) in best {
            println!("- {:?} matches to {:?} with score {}", vehicle, trip, score);
        }
    }

    pub fn score_vehicle_similarity_to_trips(&self, id: VehicleID) -> Vec<(TripID, Distance)> {
        let vehicle_trajectory = &self.vehicles[id.0].trajectory;
        let mut scores = Vec::new();
        for variant in self.vehicle_to_possible_routes(id) {
            for trip in &self.gtfs.variant(variant).trips {
                // Just see how closely the AVL matches stop position according to the timetable.
                // This'll not be a good match when there are delays.
                let mut expected = Vec::new();
                for stop_time in &trip.stop_times {
                    expected.push((
                        stop_time.arrival_time,
                        self.gtfs.stops[&stop_time.stop_id].pos,
                    ));
                }
                if let Some(score) = vehicle_trajectory.score_at_points(expected) {
                    scores.push((trip.id, score));
                }
            }
        }
        scores.sort_by_key(|pair| pair.1);
        scores
    }

    pub fn match_to_route_shapes(&self, vehicle: VehicleID) -> Result<()> {
        let list = self.vehicle_to_possible_routes(vehicle);
        if !list.is_empty() {
            // TODO Just do the first one
            self.segment_avl_by_endpoints(vehicle, list[0]);
        }
        Ok(())
    }

    pub fn segment_avl_by_endpoints(
        &self,
        vehicle: VehicleID,
        variant: RouteVariantID,
    ) -> Vec<Trajectory> {
        let variant = self.gtfs.variant(variant);
        let shape_pl = &self.gtfs.shapes[&variant.shape_id];

        let vehicle_trajectory = &self.vehicles[vehicle.0].trajectory;

        // Idea 1: find all times the AVL is close to endpoints of the shape. Use those to clip
        // into multiple pieces, just view it
        let threshold = Distance::meters(10.0);
        let times_near_start = vehicle_trajectory.times_near_pos(shape_pl.first_pt(), threshold);
        let times_near_end = vehicle_trajectory.times_near_pos(shape_pl.last_pt(), threshold);

        if true {
            println!("does {:?} match {:?}?", vehicle, variant.variant_id);
            println!("near start at:");
            for (t, _) in &times_near_start {
                println!("- {t}");
            }
            println!("near end at:");
            for (t, _) in &times_near_end {
                println!("- {t}");
            }
        }

        // Attempt to match up the times
        let mut intervals = Vec::new();
        for ((t1, _), (t2, _)) in times_near_start.into_iter().zip(times_near_end.into_iter()) {
            if t1 < t2 {
                if intervals
                    .last()
                    .map(|(_, prev_t2)| t1 > *prev_t2)
                    .unwrap_or(true)
                {
                    intervals.push((t1, t2));
                    println!("... so interval from {t1} to {t2}");
                    continue;
                }
            }
            println!("??? what happened from {t1} to {t2}");
        }

        // TODO Now clip_to_time for the good matches? Or if there's unexplainable bits, don't
        // consider this a good match?

        // Idea 2: find all times the AVL is close to each of the stops. just see what those look
        // like

        Vec::new()
    }
}
