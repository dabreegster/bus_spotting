use std::collections::BTreeMap;

use anyhow::Result;
use geom::{GPSBounds, LonLat, Pt2D};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StopID(String);

#[derive(Serialize, Deserialize)]
pub struct Stop {
    pub stop_id: StopID,
    pub pos: Pt2D,
    pub code: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
}

pub fn load<R: std::io::Read>(reader: R) -> Result<(BTreeMap<StopID, Stop>, GPSBounds)> {
    let mut gps_bounds = GPSBounds::new();
    let mut records = Vec::new();
    for rec in csv::Reader::from_reader(reader).deserialize() {
        let rec: Record = rec?;
        gps_bounds.update(LonLat::new(rec.stop_lon, rec.stop_lat));
        records.push(rec);
    }

    let mut stops = BTreeMap::new();
    for rec in records {
        if stops.contains_key(&rec.stop_id) {
            bail!("Duplicate {:?}", rec.stop_id);
        }
        stops.insert(
            rec.stop_id.clone(),
            Stop {
                stop_id: rec.stop_id,
                pos: LonLat::new(rec.stop_lon, rec.stop_lat).to_pt(&gps_bounds),
                code: rec.stop_code,
                name: rec.stop_name,
                description: rec.stop_desc,
            },
        );
    }
    Ok((stops, gps_bounds))
}

#[derive(Deserialize)]
struct Record {
    stop_id: StopID,
    stop_code: Option<String>,
    stop_name: Option<String>,
    stop_desc: Option<String>,
    stop_lon: f64,
    stop_lat: f64,
    // TODO Assuming location_type = 0 or empty
}
