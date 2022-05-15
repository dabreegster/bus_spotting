use std::collections::BTreeMap;

mod stops;

use anyhow::Result;
use geom::GPSBounds;

pub use stops::{Stop, StopID};

pub struct GTFS {
    pub stops: BTreeMap<StopID, Stop>,
}

impl GTFS {
    pub fn load_from_dir(gps_bounds: &GPSBounds, path: &str) -> Result<Self> {
        Ok(Self {
            stops: stops::load(gps_bounds, format!("{path}/stops.txt"))?,
        })
    }
}
