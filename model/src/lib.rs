#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod avl;
pub mod gtfs;
mod trajectory;

use abstutil::Timer;
use anyhow::Result;
use geom::{Bounds, GPSBounds, Pt2D};
use serde::{Deserialize, Serialize};

use self::gtfs::GTFS;
pub use self::trajectory::Trajectory;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VehicleName(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VehicleID(pub usize);

#[derive(Serialize, Deserialize)]
pub struct Model {
    pub bounds: Bounds,
    pub gps_bounds: GPSBounds,
    // TODO TiVec
    pub vehicles: Vec<Vehicle>,
    pub gtfs: GTFS,
}

#[derive(Serialize, Deserialize)]
pub struct Vehicle {
    pub id: VehicleID,
    pub original_id: VehicleName,
    pub trajectory: Trajectory,
}

impl Model {
    pub fn import_zip_bytes(bytes: Vec<u8>, timer: &mut Timer) -> Result<Self> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;

        // TODO Handle many AVL files. Use an arbitrary one for now.
        timer.start("loading AVL");
        let avl_path = archive
            .file_names()
            .find(|x| x.starts_with("avl/") && x.ends_with(".csv"))
            .unwrap()
            .to_string();
        let (gps_bounds, trajectories) = avl::load(archive.by_name(&avl_path)?)?;
        let mut vehicles = Vec::new();
        for (original_id, trajectory) in trajectories {
            vehicles.push(Vehicle {
                id: VehicleID(vehicles.len()),
                original_id,
                trajectory,
            });
        }
        timer.stop("loading AVL");

        timer.start("loading GTFS");
        let gtfs = GTFS::load_from_dir(&gps_bounds, &mut archive)?;
        timer.stop("loading GTFS");

        Ok(Self {
            bounds: gps_bounds.to_bounds(),
            gps_bounds,
            vehicles,
            gtfs,
        })
    }

    pub fn empty() -> Self {
        Self {
            // Avoid crashing the UI with empty bounds
            bounds: Bounds::from(&[Pt2D::zero(), Pt2D::new(1.0, 1.0)]),
            gps_bounds: GPSBounds::new(),
            vehicles: Vec::new(),
            gtfs: GTFS::empty(),
        }
    }
}
