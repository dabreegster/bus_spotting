use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use geom::{GPSBounds, LonLat, Pt2D};
use serde::{Deserialize, Serialize};

use super::{orig, IDMapping, RouteVariantID, StopID};

#[derive(Serialize, Deserialize)]
pub struct Stop {
    pub id: StopID,
    pub orig_id: orig::StopID,
    pub pos: Pt2D,
    pub code: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,

    // Derived data, but useful to store directly. We can consider lazily filling this out if the
    // serialized size is high.
    pub route_variants: BTreeSet<RouteVariantID>,
}

pub fn load<R: std::io::Read>(
    reader: R,
) -> Result<(
    BTreeMap<StopID, Stop>,
    IDMapping<orig::StopID, StopID>,
    GPSBounds,
)> {
    let mut gps_bounds = GPSBounds::new();
    let mut records = Vec::new();
    for rec in csv::Reader::from_reader(reader).deserialize() {
        let rec: Record = rec?;
        gps_bounds.update(LonLat::new(rec.stop_lon, rec.stop_lat));
        records.push(rec);
    }

    let mut stops = BTreeMap::new();
    let mut ids = IDMapping::new();
    for rec in records {
        let id = ids.insert_new(rec.stop_id.clone())?;
        stops.insert(
            id,
            Stop {
                id,
                orig_id: rec.stop_id,
                pos: LonLat::new(rec.stop_lon, rec.stop_lat).to_pt(&gps_bounds),
                code: rec.stop_code,
                name: rec.stop_name,
                description: rec.stop_desc,

                route_variants: BTreeSet::new(),
            },
        );
    }
    Ok((stops, ids, gps_bounds))
}

#[derive(Deserialize)]
struct Record {
    stop_id: orig::StopID,
    stop_code: Option<String>,
    stop_name: Option<String>,
    stop_desc: Option<String>,
    stop_lon: f64,
    stop_lat: f64,
    // TODO Assuming location_type = 0 or empty
}
