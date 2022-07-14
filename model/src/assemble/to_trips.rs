use geom::{Distance, Duration, Time};

use crate::{Model, Timetable, VehicleID};
use gtfs::{RouteVariantID, TripID};

pub const BUS_TO_STOP_THRESHOLD: Distance = Distance::const_meters(30.0);

impl Model {
    /// Given one vehicle, use `get_trips_for_vehicle_and_variant` against all possible variants,
    /// then merge the results into one schedule through the day. Returns non-overlapping trips in
    /// order.
    ///
    /// Note we don't check that the same TripID isn't covered by two different vehicles. This
    /// method looks at one vehicle only.
    pub fn infer_vehicle_schedule(&self, vehicle: VehicleID, debug: bool) -> Vec<ActualTrip> {
        let mut all_possible_trips = Vec::new();
        for variant in self.vehicle_to_possible_routes(vehicle) {
            all_possible_trips.extend(self.get_trips_for_vehicle_and_variant(vehicle, variant));
        }

        // Walk through in order of start time. Greedily add a trip if the time intervals don't
        // overlap.
        //
        // Long trips (usually buggy) often win.
        if false {
            all_possible_trips.sort_by_key(|t| t.start_time());
            let mut final_schedule: Vec<ActualTrip> = Vec::new();
            for trip in all_possible_trips {
                if final_schedule
                    .last()
                    .as_ref()
                    .map(|last| last.end_time() < trip.start_time())
                    .unwrap_or(true)
                {
                    final_schedule.push(trip);
                } else if debug {
                    println!("Skipping {}", trip.summary());
                }
            }
            return final_schedule;
        }

        // Sort by trip duration, then insert those into a schedule as they fit.
        all_possible_trips.sort_by_key(|t| t.end_time() - t.start_time());
        let mut timetable = Timetable::new();
        for trip in all_possible_trips {
            if timetable.is_free((trip.start_time(), trip.end_time())) {
                timetable.assign((trip.start_time(), trip.end_time()), trip);
            } else if debug {
                println!("Skipping {}", trip.summary());
            }
        }
        timetable.0.into_iter().map(|(_, _, trip)| trip).collect()
    }

    /// Given a vehicle and one variant it possibly serves (according to ticketing), match its
    /// trajectory to all stops along that variant. Find all times it passes close to each stop,
    /// then assemble those into a likely sequence of trips serving that variant.
    ///
    /// The result mostly looks good, but the distance threshold still needs tuning. Strange
    /// results can generally be detected from very long trip times.
    pub fn get_trips_for_vehicle_and_variant(
        &self,
        vehicle: VehicleID,
        variant: RouteVariantID,
    ) -> Vec<ActualTrip> {
        let trips = self.get_trip_times(vehicle, variant);

        let gtfs_trips = &self.gtfs.variant(variant).trips;
        if trips.len() > gtfs_trips.len() {
            warn!(
                "For {:?}, found {} actual trips, but GTFS only has {}",
                variant,
                trips.len(),
                gtfs_trips.len()
            );
        }

        let mut results = Vec::new();

        for stop_times in trips {
            // Which GTFS trip is this? If all scheduled trips occurred, we could just match them
            // up in order, but that's rarely the case. Minimize the sum of time differences over
            // all stops.
            let trip = gtfs_trips
                .iter()
                .min_by_key(|trip| {
                    let mut sum_diff = Duration::ZERO;
                    for (actual_time, stop_time) in stop_times.iter().zip(trip.stop_times.iter()) {
                        sum_diff += (*actual_time - stop_time.arrival_time).abs();
                    }
                    sum_diff
                })
                .map(|trip| trip.id)
                .unwrap();
            results.push(ActualTrip {
                vehicle,
                variant,
                trip,
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
            let stop_pos = self.gtfs.stops[&stop].pos;
            let times: Vec<Time> = trajectory
                .times_near_pos(stop_pos, BUS_TO_STOP_THRESHOLD)
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

impl ActualTrip {
    pub fn summary(&self) -> String {
        format!(
            "{:?} ({:?}) from {} to {} ({} total)",
            self.trip,
            self.variant,
            self.stop_times[0],
            self.stop_times.last().unwrap(),
            *self.stop_times.last().unwrap() - self.stop_times[0]
        )
    }

    pub fn show_schedule(&self) -> Vec<String> {
        let mut out = Vec::new();

        // More compressed, but harder to read
        if false {
            out.push(format!(
                "- Trip: {}",
                self.stop_times
                    .iter()
                    .enumerate()
                    .map(|(idx, t)| format!("{} @ {}", idx + 1, t))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));

            // Look for impossible bits, show the stops
            for (idx, pair) in self.stop_times.windows(2).enumerate() {
                if pair[1] < pair[0] {
                    out.push(format!(
                        "  - Something funny near stop {} ({}) -> {} ({})",
                        idx + 1,
                        pair[0],
                        idx + 2,
                        pair[1]
                    ));
                }
            }
        }

        let mut last_time = self.stop_times[0];
        for (idx, time) in self.stop_times.iter().enumerate() {
            let time = *time;
            out.push(format!(
                "  Stop {}: {} ({})",
                idx + 1,
                time,
                time - last_time
            ));
            last_time = time;
        }
        out
    }

    pub fn start_time(&self) -> Time {
        self.stop_times[0]
    }

    pub fn end_time(&self) -> Time {
        *self.stop_times.last().unwrap()
    }
}
