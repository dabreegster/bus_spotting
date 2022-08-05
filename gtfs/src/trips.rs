use anyhow::Result;
use geom::Time;
use serde::{Deserialize, Serialize};

use super::{orig, IDMapping, RouteID, ServiceID, ShapeID, StopID, StopTime, TripID};

#[derive(Clone, Serialize, Deserialize)]
pub struct Trip {
    pub id: TripID,
    pub orig_id: orig::TripID,
    pub route_id: RouteID,
    pub shape_id: ShapeID,
    pub service_id: ServiceID,
    pub headsign: Option<String>,
    /// true is 0 in GTFS, false is 1. Inbound/outbound are arbitrary.
    pub outbound_direction: bool,

    pub stop_times: Vec<StopTime>,
}

impl Trip {
    /// Panics if this trip doesn't visit this stop. Assumes the trip doesn't visit the same stop
    /// twice.
    pub fn arrival_at(&self, stop_id: StopID) -> Time {
        for st in &self.stop_times {
            if st.stop_id == stop_id {
                return st.arrival_time;
            }
        }
        panic!("{:?} doesn't visit {:?}", self.orig_id, stop_id);
    }

    pub fn time_range(&self) -> (Time, Time) {
        (
            self.stop_times[0].arrival_time,
            self.stop_times.last().unwrap().departure_time,
        )
    }
}

pub fn load<R: std::io::Read>(reader: R) -> Result<(Vec<Trip>, IDMapping<orig::TripID, TripID>)> {
    let mut trips = Vec::new();
    let mut ids = IDMapping::new();
    for rec in csv::Reader::from_reader(reader).deserialize() {
        let rec: Record = rec?;
        let id = ids.insert_new(rec.trip_id.clone())?;
        trips.push(Trip {
            id,
            orig_id: rec.trip_id,
            route_id: rec.route_id,
            shape_id: rec.shape_id,
            service_id: rec.service_id,
            headsign: rec.trip_headsign,
            outbound_direction: match rec.direction_id {
                Some(0) => true,
                Some(1) => false,
                // outbound_direction is just used for grouping, so if there's no direction, that's
                // fine
                None => true,
                x => bail!("Unknown direction_id {:?}", x),
            },

            stop_times: Vec::new(),
        });
    }
    Ok((trips, ids))
}

#[derive(Deserialize)]
struct Record {
    trip_id: orig::TripID,
    route_id: RouteID,
    trip_headsign: Option<String>,
    direction_id: Option<usize>,
    shape_id: ShapeID,
    service_id: ServiceID,
}
