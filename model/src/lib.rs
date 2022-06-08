#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod avl;
pub mod gtfs;
mod ticketing;
mod trajectory;

use abstutil::Timer;
use anyhow::Result;
use geom::{Bounds, GPSBounds, Pt2D};
use serde::{Deserialize, Serialize};

use self::gtfs::GTFS;
pub use self::ticketing::{CardID, Journey, JourneyLeg};
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
    pub journeys: Vec<Journey>,
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

        timer.start("loading GTFS");
        let (gtfs, gps_bounds) = GTFS::load_from_dir(&mut archive)?;
        timer.stop("loading GTFS");

        // TODO Handle many AVL files. Use an arbitrary one for now.
        timer.start("loading AVL");
        let avl_path = archive
            .file_names()
            .find(|x| x.starts_with("avl/") && x.ends_with(".csv"))
            .unwrap()
            .to_string();
        let trajectories = avl::load(archive.by_name(&avl_path)?, &gps_bounds)?;
        let mut vehicles = Vec::new();
        for (original_id, trajectory) in trajectories {
            vehicles.push(Vehicle {
                id: VehicleID(vehicles.len()),
                original_id,
                trajectory,
            });
        }
        timer.stop("loading AVL");

        timer.start("loading BIL");
        let bil_path = archive
            .file_names()
            .find(|x| x.starts_with("bil/") && x.ends_with(".csv"))
            .unwrap()
            .to_string();
        let journeys = ticketing::load(archive.by_name(&bil_path)?, &gps_bounds)?;
        timer.stop("loading BIL");

        Ok(Self {
            bounds: gps_bounds.to_bounds(),
            gps_bounds,
            vehicles,
            gtfs,
            journeys,
        })
    }

    pub fn empty() -> Self {
        Self {
            // Avoid crashing the UI with empty bounds
            bounds: Bounds::from(&[Pt2D::zero(), Pt2D::new(1.0, 1.0)]),
            gps_bounds: GPSBounds::new(),
            vehicles: Vec::new(),
            gtfs: GTFS::empty(),
            journeys: Vec::new(),
        }
    }
}
