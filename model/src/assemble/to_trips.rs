use geom::{Distance, Time};

use crate::gtfs::{RouteVariantID, TripID};
use crate::{Model, VehicleID};

impl Model {
    // TODO This assumes a single variant. Next step is to look at all possibilities and remove
    // overlaps between them automatically
    pub fn get_trips_for_vehicle_and_variant(
        &self,
        vehicle: VehicleID,
        variant: RouteVariantID,
    ) -> Vec<ActualTrip> {
        let trips = self.get_trip_times(vehicle, variant);
        print_timetable(trips.clone());

        // Match to actual trip IDs in order. Don't attempt to match up times at all yet.
        let gtfs_trips = &self.gtfs.variant(variant).trips;
        if trips.len() > gtfs_trips.len() {
            println!(
                "Found {} actual trips, but GTFS only has {}",
                trips.len(),
                gtfs_trips.len()
            );
        }

        let mut results = Vec::new();
        for (stop_times, gtfs_trip) in trips.into_iter().zip(gtfs_trips.into_iter()) {
            results.push(ActualTrip {
                vehicle,
                variant,
                trip: gtfs_trip.id,
                stop_times,
            });
        }

        results
    }

    // Look for all times the vehicle passes close to each stop. Then assemble those into trip
    // sequences, forcing times to be in order.
    //
    // Errors seem to happen when the distance threshold is too low, and other cases not yet
    // understood. Simple validation is to look for huge times between stops (over an hour).
    fn get_trip_times(&self, vehicle: VehicleID, variant: RouteVariantID) -> Vec<Vec<Time>> {
        let trajectory = &self.vehicles[vehicle.0].trajectory;
        let variant = self.gtfs.variant(variant);

        let mut times_near_stops: Vec<Vec<Time>> = Vec::new();
        let mut min_times = usize::MAX;
        for stop in variant.stops() {
            let threshold = Distance::meters(10.0);
            let stop_pos = self.gtfs.stops[&stop].pos;
            let times: Vec<Time> = trajectory
                .times_near_pos(stop_pos, threshold)
                .into_iter()
                .map(|(t, _)| t)
                .collect();
            min_times = min_times.min(times.len());
            times_near_stops.push(times);
        }

        // Assemble into trips
        let mut trips: Vec<Vec<Time>> = Vec::new();

        if false {
            // The naive approach
            for trip_idx in 0..min_times {
                let times: Vec<Time> = times_near_stops
                    .iter()
                    .map(|times| times[trip_idx])
                    .collect();
                trips.push(times);
            }
        } else {
            // Assume the first time at the first stop is correct, then build up from there and always
            // require the time to increase. Skip some times if needed
            let mut skipped = 0;
            let mut last_time = Time::START_OF_DAY;
            'OUTER: loop {
                let mut trip_times = Vec::new();
                for times in &mut times_near_stops {
                    // Shift while the first time is too early
                    while !times.is_empty() && times[0] < last_time {
                        times.remove(0);
                        skipped += 1;
                    }
                    if times.is_empty() {
                        break 'OUTER;
                    }
                    last_time = times.remove(0);
                    trip_times.push(last_time);
                }
                trips.push(trip_times);
            }

            if false {
                println!(
                    "For below, skipped {} times at different stops because they're out-of-order",
                    skipped
                );
            }
        }

        trips
    }
}

pub struct ActualTrip {
    // For convenience
    pub vehicle: VehicleID,
    pub variant: RouteVariantID,
    pub trip: TripID,

    pub stop_times: Vec<Time>,
}

fn print_timetable(trips: Vec<Vec<Time>>) {
    println!("{} trips", trips.len());
    for times in trips {
        // More compressed, but harder to read
        if false {
            println!(
                "- Trip: {}",
                times
                    .iter()
                    .enumerate()
                    .map(|(idx, t)| format!("{} @ {}", idx + 1, t))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            // Look for impossible bits, show the stops
            for (idx, pair) in times.windows(2).enumerate() {
                if pair[1] < pair[0] {
                    println!(
                        "  - Something funny near stop {} ({}) -> {} ({})",
                        idx + 1,
                        pair[0],
                        idx + 2,
                        pair[1]
                    );
                }
            }
        }

        println!(
            "--- Trip from {} to {} ({} total)",
            times[0],
            times.last().unwrap(),
            *times.last().unwrap() - times[0]
        );
        let mut last_time = times[0];
        for (idx, time) in times.into_iter().enumerate() {
            println!("  Stop {}: {} ({})", idx + 1, time, time - last_time);
            last_time = time;
        }
    }
}
