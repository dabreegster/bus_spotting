use std::collections::{BTreeMap, BTreeSet};

use abstutil::{prettyprint_usize, Timer};
use anyhow::Result;
use geom::{Histogram, Time};
use serde::{Deserialize, Serialize};

use crate::{JourneyID, Model, Timetable, VehicleID};
use gtfs::{RouteVariantID, StopID, TripID};

// TODO UIs
// - for just a variant (click in the world)
//   - how many diff vehicles serve it?
//   - make a graph. X axis time, Y axis stops (spaced by distance along shape?)
//     - one line for each vehicle. dot per stop, with color/size/tooltip showing boardings

// Represents a vehicle arriving at a stop, and maybe people boarding (either as new riders or
// transfers). This effectively joins AVL, BIL, and GTFS.
//
// Most of the high-level UI could be built on top of this, and the data model could omit all the
// raw AVL and BIL data.
//
// Uniquely keyed by (TripID, StopID)
#[derive(Serialize, Deserialize)]
pub struct BoardingEvent {
    pub vehicle: VehicleID,
    // For convenience
    pub variant: RouteVariantID,
    pub trip: TripID,
    pub stop: StopID,
    pub arrival_time: Time,
    pub departure_time: Time,
    pub new_riders: Vec<JourneyID>,
    pub transfers: Vec<JourneyID>,
}

// It's basically SQL at this point, happy?
impl Model {
    pub fn find_boarding_event(&self, trip: TripID, stop: StopID) -> Option<&BoardingEvent> {
        self.boardings
            .iter()
            .find(|ev| ev.trip == trip && ev.stop == stop)
    }

    pub fn boarding_event_for_vehicle_stop_time(
        &self,
        vehicle: VehicleID,
        stop: StopID,
        time: Time,
    ) -> Option<&BoardingEvent> {
        self.boardings
            .iter()
            .find(|ev| ev.vehicle == vehicle && ev.stop == stop && ev.arrival_time == time)
    }

    pub fn most_recent_boarding_event_for_bus(
        &self,
        vehicle: VehicleID,
        time: Time,
    ) -> Option<&BoardingEvent> {
        self.boardings
            .iter()
            .rev()
            .find(|ev| ev.vehicle == vehicle && ev.arrival_time <= time)
    }

    pub fn all_boarding_events_at_stop(&self, stop: StopID) -> Vec<&BoardingEvent> {
        self.boardings.iter().filter(|ev| ev.stop == stop).collect()
    }
}

pub fn populate_boarding(model: &mut Model, timer: &mut Timer) -> Result<()> {
    timer.start("populate_boarding");

    // Every stop a vehicle visits through the day, in order
    let mut events_per_vehicle: BTreeMap<VehicleID, Vec<BoardingEvent>> = BTreeMap::new();

    // Fill out empty BoardingEvents for each stop along each trip
    let mut all_trip_durations = Histogram::new();
    for (vehicle, events, trip_durations, timetable) in timer.parallelize(
        "calculate schedule for vehicles",
        model.vehicles.iter().map(|v| v.id).collect(),
        |vehicle| {
            let mut events = Vec::new();
            let mut trip_durations = Vec::new();
            // Build up a summary timetable
            let mut timetable = Timetable::new();

            let debug = false;
            for trip in model.infer_vehicle_schedule(vehicle, debug) {
                timetable.assign((trip.start_time(), trip.end_time()), trip.trip);

                let variant = model.gtfs.variant(trip.variant);
                assert_eq!(trip.stop_times.len(), variant.stops().len());
                trip_durations.push(trip.end_time() - trip.start_time());
                for (time, stop) in trip.stop_times.into_iter().zip(variant.stops().into_iter()) {
                    events.push(BoardingEvent {
                        vehicle: trip.vehicle,
                        variant: trip.variant,
                        trip: trip.trip,
                        stop,
                        arrival_time: time,
                        departure_time: time,
                        new_riders: Vec::new(),
                        transfers: Vec::new(),
                    });
                }
            }
            (vehicle, events, trip_durations, timetable)
        },
    ) {
        events_per_vehicle.insert(vehicle, events);
        for d in trip_durations {
            all_trip_durations.add(d);
        }

        model.vehicles[vehicle.0].timetable = timetable;
    }

    // Sanity check multiple vehicles aren't assigned to the same trip.
    let mut trip_to_vehicles: BTreeMap<TripID, BTreeSet<VehicleID>> = BTreeMap::new();
    for events in events_per_vehicle.values() {
        for event in events {
            trip_to_vehicles
                .entry(event.trip)
                .or_insert_with(BTreeSet::new)
                .insert(event.vehicle);
        }
    }
    let mut trip_problems = 0;
    let total_found_trips = trip_to_vehicles.len();
    for (trip, vehicles) in trip_to_vehicles {
        if vehicles.len() > 1 {
            trip_problems += 1;
            error!(
                "{:?} is assigned to multiple vehicles: {:?}",
                trip, vehicles
            );
        }
    }
    error!("{} trips with multiple vehicles", trip_problems);

    // Debug one vehicle
    let debug_vehicle = VehicleID(220);
    if false {
        info!("Debugging events for {:?}", debug_vehicle);
        for ev in &events_per_vehicle[&debug_vehicle] {
            info!("  {}", ev.arrival_time);
        }
    }

    // Match each ticketing event to the appropriate vehicle. Assume people tap on AFTER boarding
    // the bus and match to the most recent stop time.
    let mut matched_events = 0;
    let mut unmatched_events = 0;

    let mut route_name_mismatches = 0;
    let mut delay_before_ticketing = Histogram::new();
    let mut stop_dist_to_ticketing = Histogram::new();
    for (journey_idx, journey) in model.journeys.iter().enumerate() {
        for (leg_idx, leg) in journey.legs.iter().enumerate() {
            let mut ok = false;

            if let Ok(vehicle) = model.vehicle_ids.lookup(&leg.vehicle_name) {
                // Check stops in reverse, so we can find the first stop occurring before this
                // ticketing event
                for event in events_per_vehicle
                    .get_mut(&vehicle)
                    .unwrap()
                    .iter_mut()
                    .rev()
                {
                    if leg.time >= event.arrival_time {
                        // Sanity check: does the route on the ticketing event match what we've
                        // assigned the vehicle?
                        let vehicle_route = model.gtfs.routes
                            [&model.gtfs.variant(event.variant).route_id]
                            .short_name
                            .as_ref()
                            .unwrap();
                        if &leg.route_short_name != vehicle_route {
                            route_name_mismatches += 1;
                        }

                        // Sanity check: the ticketing event should happen shortly after the
                        // vehicle arrives at the stop
                        delay_before_ticketing.add(leg.time - event.arrival_time);

                        // Sanity check: the ticketing event shouldn't occur too far physically
                        // from the stop
                        stop_dist_to_ticketing
                            .add(leg.pos.dist_to(model.gtfs.stops[&event.stop].pos));

                        ok = true;
                        if leg_idx == 0 {
                            event.new_riders.push(JourneyID(journey_idx));
                        } else {
                            event.transfers.push(JourneyID(journey_idx));
                        }

                        if false && vehicle == debug_vehicle {
                            info!("... someone boards that vehicle at {}. {} after arrival at stop, and {} away", leg.time, leg.time - event.arrival_time, leg.pos.dist_to(model.gtfs.stops[&event.stop].pos));
                        }

                        break;
                    }
                }
            }

            if ok {
                matched_events += 1;
            } else {
                unmatched_events += 1;
            }
        }
    }

    info!("Final model quality");
    let total_expected_gtfs_trips = model
        .get_gtfs_trip_demand()
        .into_values()
        .map(|list| list.len())
        .sum();
    println!(
        "{} total trips matched (GTFS says to expect {})",
        prettyprint_usize(total_found_trips),
        prettyprint_usize(total_expected_gtfs_trips)
    );
    println!("Trip durations: {}", all_trip_durations.describe());

    println!(
        "{} ticketing events matched to actual trips. {} unmatched",
        prettyprint_usize(matched_events),
        prettyprint_usize(unmatched_events)
    );
    println!(
        "Of the matched, {} don't actually match the route name",
        prettyprint_usize(route_name_mismatches)
    );
    println!(
        "Of the matched, how long between the bus arriving and the ticketing? {}",
        delay_before_ticketing.describe()
    );
    println!(
        "Of the matched, how far between the bus stop and the ticketing event? {}",
        stop_dist_to_ticketing.describe()
    );

    // Flatten (not sure how boarding events will be used yet; this is obviously not the final
    // structure)
    for (_, events) in events_per_vehicle {
        model.boardings.extend(events);
    }
    model.boardings.sort_by_key(|ev| ev.arrival_time);

    timer.stop("populate_boarding");
    Ok(())
}
