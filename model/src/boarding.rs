use abstutil::Timer;
use anyhow::Result;
use geom::{Duration, Time};
use serde::{Deserialize, Serialize};

use crate::gtfs::{RouteVariantID, StopID, TripID};
use crate::{JourneyID, Model, VehicleID};

// This effectively joins AVL, BIL, and GTFS.
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
    pub journeys: Vec<JourneyID>,
}

// Assuming we can produce this, start some UIs:
//
// - for a stop + variant...
//   - on the schedule tab, show actual/expected time (with an 'early' / 'late' indicator)
//   - make a new tab for ridership. list each of these events and count the boardings. breakdown
//     by transfer or not.
//
// - for just a variant (click in the world)
//   - how many diff vehicles serve it?
//   - make a graph. X axis time, Y axis stops (spaced by distance along shape?)
//     - one line for each vehicle. dot per stop, with color/size/tooltip showing boardings

// This is a placeholder for much more correct matching
pub fn populate(model: &mut Model, timer: &mut Timer) -> Result<()> {
    let vehicle_and_variant = model.segment(timer)?;

    // TODO Just make up some data for the moment, to start the UIs
    for (vehicle_id, variant_id) in vehicle_and_variant {
        let variant = model.gtfs.variant(variant_id);
        println!(
            "We've decided {:?} serves {}",
            vehicle_id,
            variant.describe(&model.gtfs)
        );
        for trip in &variant.trips {
            for stop_time in &trip.stop_times {
                model.boardings.push(BoardingEvent {
                    vehicle: vehicle_id,
                    variant: variant_id,
                    trip: trip.trip_id.clone(),
                    stop: stop_time.stop_id,
                    arrival_time: stop_time.arrival_time + Duration::seconds(15.0),
                    departure_time: stop_time.departure_time + Duration::seconds(25.0),
                    journeys: vec![JourneyID(0), JourneyID(1)],
                });
            }
        }
    }

    Ok(())
}

impl Model {
    pub fn find_event(&self, trip: &TripID, stop: StopID) -> Option<&BoardingEvent> {
        self.boardings
            .iter()
            .find(|ev| &ev.trip == trip && ev.stop == stop)
    }
}
