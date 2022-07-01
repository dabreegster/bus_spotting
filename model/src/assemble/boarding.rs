use abstutil::Timer;
use anyhow::Result;
use geom::{Duration, Time};
use serde::{Deserialize, Serialize};

use crate::gtfs::{RouteVariantID, StopID, TripID};
use crate::{JourneyID, Model, VehicleID};

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

impl Model {
    pub fn find_boarding_event(&self, trip: TripID, stop: StopID) -> Option<&BoardingEvent> {
        self.boardings
            .iter()
            .find(|ev| ev.trip == trip && ev.stop == stop)
    }
}

// This is a placeholder for much more correct matching
pub fn populate_boarding(model: &mut Model, _timer: &mut Timer) -> Result<()> {
    // TODO Just make up some data for the moment, to start the UIs
    for (vehicle_id, mut variants) in model.vehicles_to_possible_routes()? {
        let variant = model.gtfs.variant(variants.pop().unwrap());
        println!(
            "We've decided {:?} serves {}",
            vehicle_id,
            variant.describe(&model.gtfs)
        );
        for trip in &variant.trips {
            for stop_time in &trip.stop_times {
                model.boardings.push(BoardingEvent {
                    vehicle: vehicle_id,
                    variant: variant.variant_id,
                    trip: trip.id,
                    stop: stop_time.stop_id,
                    arrival_time: stop_time.arrival_time + Duration::seconds(15.0),
                    departure_time: stop_time.departure_time + Duration::seconds(25.0),
                    new_riders: vec![JourneyID(0), JourneyID(1)],
                    transfers: vec![JourneyID(2)],
                });
            }
        }
    }

    Ok(())
}
