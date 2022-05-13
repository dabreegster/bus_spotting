use std::collections::BTreeMap;

use anyhow::Result;
use geom::{Bounds, GPSBounds};
use serde::Deserialize;

use crate::Trajectory;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub struct VehicleID(String);

pub struct Model {
    pub bounds: Bounds,
    pub gps_bounds: GPSBounds,
    pub bus_trajectories: BTreeMap<VehicleID, Trajectory>,
}

impl Model {
    pub fn load(avl_path: &str) -> Result<Self> {
        let (gps_bounds, bus_trajectories) = crate::avl::load(avl_path)?;
        Ok(Self {
            bounds: gps_bounds.to_bounds(),
            gps_bounds,
            bus_trajectories,
        })
    }
}
