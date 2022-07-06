use std::collections::{BTreeMap, BTreeSet};

use abstutil::{prettyprint_usize, Timer};
use anyhow::Result;
use geom::{Histogram, Time};
use serde::{Deserialize, Serialize};

use crate::gtfs::{RouteVariantID, StopID, TripID};
use crate::{JourneyID, Model, VehicleID};

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

impl Model {
    pub fn find_boarding_event(&self, trip: TripID, stop: StopID) -> Option<&BoardingEvent> {
        self.boardings
            .iter()
            .find(|ev| ev.trip == trip && ev.stop == stop)
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
    for (vehicle, events) in timer.parallelize(
        "calculate schedule for vehicles",
        model.vehicles.iter().map(|v| v.id).collect(),
        |vehicle| {
            let mut events = Vec::new();
            for trip in model.infer_vehicle_schedule(vehicle) {
                let variant = model.gtfs.variant(trip.variant);
                assert_eq!(trip.stop_times.len(), variant.stops().len());
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
            (vehicle, events)
        },
    ) {
        events_per_vehicle.insert(vehicle, events);
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

    // Match each ticketing event to the appropriate vehicle. Assume people tap on AFTER boarding
    // the bus and match to the most recent stop time.
    let mut matched_events = 0;
    let mut unmatched_events = 0;

    let mut route_name_mismatches = 0;
    let mut delay_before_ticketing = Histogram::new();
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
                    if leg.time <= event.arrival_time {
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
                        delay_before_ticketing.add(event.arrival_time - leg.time);

                        ok = true;
                        if leg_idx == 0 {
                            event.new_riders.push(JourneyID(journey_idx));
                        } else {
                            event.transfers.push(JourneyID(journey_idx));
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

    info!(
        "{} ticketing events matched to actual trips. {} unmatched",
        prettyprint_usize(matched_events),
        prettyprint_usize(unmatched_events)
    );
    info!(
        "Of the matched, {} don't actually match the route name",
        prettyprint_usize(route_name_mismatches)
    );
    info!(
        "Of the matched, how long between the bus arriving and the ticketing? {}",
        delay_before_ticketing.describe()
    );

    // Flatten (not sure how boarding events will be used yet; this is obviously not the final
    // structure)
    for (_, events) in events_per_vehicle {
        model.boardings.extend(events);
    }

    timer.stop("populate_boarding");
    Ok(())
}
