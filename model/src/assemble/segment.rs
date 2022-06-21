use abstutil::Timer;
use anyhow::Result;
use geom::{Distance, Time};

use crate::gtfs::{DateFilter, RouteVariant, RouteVariantID};
use crate::{Model, VehicleID};

// TODO This approach seems too brittle. Snapping to the trajectory is messy, so trying to do it
// for boarding events is unnecessary trouble.

impl Model {
    pub fn segment(&self, timer: &mut Timer) -> Result<()> {
        // We're assuming the model only represents one day right now

        timer.start("match vehicles to route_short_name");
        let vehicles = self.vehicle_to_route_short_name()?;
        timer.stop("match vehicles to route_short_name");

        timer.start_iter("match vehicles to route variants", vehicles.len());
        // TODO This repeats part of vehicle_to_possible_routes because we need assignments
        let services = self
            .gtfs
            .calendar
            .services_matching_dates(&DateFilter::SingleDay(self.main_date));
        for (vehicle, assignment) in &vehicles {
            timer.next();

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
                    *vehicle,
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

struct PossibleMatch {
    _vehicle: VehicleID,
    _variant: RouteVariantID,
    // For each boarding event, find the closest point on the variant's shape and get the distance
    // along of that. Sorted by time.
    boardings: Vec<(Time, Distance)>,
}

impl PossibleMatch {
    fn new(
        model: &Model,
        vehicle: VehicleID,
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
                    // TODO When shapes double back on themselves, this will likely oscillate in
                    // weird ways
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
            _vehicle: vehicle,
            _variant: variant.variant_id,
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
