use std::collections::BTreeMap;

use anyhow::Result;
use fs_err::File;
use serde::{Deserialize, Serialize};

use super::{RouteID, ShapeID};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TripID(String);

#[derive(Serialize, Deserialize)]
pub struct Trip {
    pub trip_id: TripID,
    pub route_id: RouteID,
    pub shape_id: ShapeID,
    pub headsign: Option<String>,
    /// true is 0 in GTFS, false is 1. Inbound/outbound are arbitrary.
    pub outbound_direction: bool,
}

pub fn load(path: String) -> Result<BTreeMap<TripID, Trip>> {
    let mut trips = BTreeMap::new();
    for rec in csv::Reader::from_reader(File::open(path)?).deserialize() {
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
                headsign: rec.trip_headsign,
                outbound_direction: match rec.direction_id {
                    0 => true,
                    1 => false,
                    x => bail!("Unknown direction_id {x}"),
                },
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
}
