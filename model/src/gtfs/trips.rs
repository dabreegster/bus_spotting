use std::collections::BTreeMap;

use anyhow::Result;
use geom::Time;
use serde::{Deserialize, Serialize};

use super::{RouteID, ServiceID, ShapeID, StopID, StopTime};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TripID(String);

#[derive(Serialize, Deserialize)]
pub struct Trip {
    pub trip_id: TripID,
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
        panic!("{:?} doesn't visit {:?}", self.trip_id, stop_id);
    }
}

pub fn load<R: std::io::Read>(reader: R) -> Result<BTreeMap<TripID, Trip>> {
    let mut trips = BTreeMap::new();
    for rec in csv::Reader::from_reader(reader).deserialize() {
        let rec: Record = rec?;
        if trips.contains_key(&rec.trip_id) {
            bail!("Duplicate {:?}", rec.trip_id);
        }
        trips.insert(
            rec.trip_id.clone(),
            Trip {
                trip_id: rec.trip_id,
                route_id: rec.route_id,
                shape_id: rec.shape_id,
                service_id: rec.service_id,
                headsign: rec.trip_headsign,
                outbound_direction: match rec.direction_id {
                    0 => true,
                    1 => false,
                    x => bail!("Unknown direction_id {x}"),
                },

                stop_times: Vec::new(),
            },
        );
    }
    Ok(trips)
}

#[derive(Deserialize)]
struct Record {
    trip_id: TripID,
    route_id: RouteID,
    trip_headsign: Option<String>,
    direction_id: usize,
    shape_id: ShapeID,
    service_id: ServiceID,
}
