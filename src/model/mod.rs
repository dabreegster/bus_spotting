mod avl;
mod trajectory;

use anyhow::Result;
use geom::{Bounds, GPSBounds};
use serde::Deserialize;

pub use self::trajectory::Trajectory;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub struct VehicleName(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VehicleID(pub usize);

pub struct Model {
    pub bounds: Bounds,
    pub gps_bounds: GPSBounds,
    // TODO TiVec
    pub vehicles: Vec<Vehicle>,
}

pub struct Vehicle {
    pub id: VehicleID,
    pub original_id: VehicleName,
    pub trajectory: Trajectory,
}

impl Model {
    pub fn load(avl_path: &str) -> Result<Self> {
        let (gps_bounds, trajectories) = avl::load(avl_path)?;
        let mut vehicles = Vec::new();
        for (original_id, trajectory) in trajectories {
            vehicles.push(Vehicle {
                id: VehicleID(vehicles.len()),
                original_id,
                trajectory,
            });
        }
        Ok(Self {
            bounds: gps_bounds.to_bounds(),
            gps_bounds,
            vehicles,
        })
    }
}