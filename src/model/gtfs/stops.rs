use std::collections::BTreeMap;

use anyhow::Result;
use fs_err::File;
use geom::{GPSBounds, LonLat, Pt2D};
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub struct StopID(String);

pub struct Stop {
    pub stop_id: StopID,
    pub pos: Pt2D,
    pub code: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
}

pub fn load(gps_bounds: &GPSBounds, path: String) -> Result<BTreeMap<StopID, Stop>> {
    let mut stops = BTreeMap::new();
    for rec in csv::Reader::from_reader(File::open(path)?).deserialize() {
        let rec: Record = rec?;
        if stops.contains_key(&rec.stop_id) {
            bail!("Duplicate {:?}", rec.stop_id);
        }
        stops.insert(
            rec.stop_id.clone(),
            Stop {
                stop_id: rec.stop_id,
                pos: LonLat::new(rec.stop_lon, rec.stop_lat).to_pt(gps_bounds),
                code: rec.stop_code,
                name: rec.stop_name,
                description: rec.stop_desc,
            },
        );
    }
    Ok(stops)
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
