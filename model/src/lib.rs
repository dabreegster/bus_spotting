#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod avl;
pub mod gtfs;
mod segment;
mod ticketing;
mod trajectory;

use abstutil::Timer;
use anyhow::Result;
use chrono::NaiveDate;
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

    // If we've loaded journey and vehicle data, this is the one day covered. If not, it's an
    // arbitrary date covered by some GTFS service.
    pub main_date: NaiveDate,
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

        let mut main_date = gtfs.calendar.services.values().next().unwrap().start_date;

        // TODO Handle many AVL files. Use an arbitrary one for now.
        let mut vehicles = Vec::new();
        timer.start("loading AVL");
        // Indirection for the borrow checker
        let maybe_avl_path = archive
            .file_names()
            .find(|x| x.starts_with("avl/") && x.ends_with(".csv"))
            .map(|x| x.to_string());
        if let Some(avl_path) = maybe_avl_path {
            let (trajectories, avl_date) = avl::load(archive.by_name(&avl_path)?, &gps_bounds)?;
            main_date = avl_date;
            for (original_id, trajectory) in trajectories {
                vehicles.push(Vehicle {
                    id: VehicleID(vehicles.len()),
                    original_id,
                    trajectory,
                });
            }
        }
        timer.stop("loading AVL");

        let mut journeys = Vec::new();
        timer.start("loading BIL");
        let maybe_bil_path = archive
            .file_names()
            .find(|x| x.starts_with("bil/") && x.ends_with(".csv"))
            .map(|x| x.to_string());
        if let Some(bil_path) = maybe_bil_path {
            let (j, journey_day) = ticketing::load(archive.by_name(&bil_path)?, &gps_bounds)?;
            journeys = j;
            if journey_day != main_date {
                bail!("BIL date is {journey_day}, but AVL date is {main_date}");
            }
        }
        timer.stop("loading BIL");

        Ok(Self {
            bounds: gps_bounds.to_bounds(),
            gps_bounds,
            vehicles,
            gtfs,
            journeys,
            main_date,
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
            main_date: NaiveDate::from_ymd(2020, 1, 1),
        }
    }

    pub fn lookup_vehicle(&self, name: &VehicleName) -> Option<&Vehicle> {
        self.vehicles.iter().find(|v| &v.original_id == name)
    }
}
