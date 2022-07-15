#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod assemble;
mod avl;
mod experiments;
mod ticketing;
mod timetable;
mod trajectory;

use std::collections::BTreeMap;

use abstutil::Timer;
use anyhow::Result;
use chrono::NaiveDate;
use geom::{Bounds, GPSBounds, Pt2D};
use serde::{Deserialize, Serialize};

use gtfs::{IDMapping, GTFS};

pub use self::assemble::*;
pub use self::ticketing::{CardID, Journey, JourneyID, JourneyLeg};
pub use self::timetable::Timetable;
pub use self::trajectory::Trajectory;

// TODO Rearrange some of this as a DailyModel?
#[derive(Serialize, Deserialize)]
pub struct Model {
    pub bounds: Bounds,
    pub gps_bounds: GPSBounds,
    // TODO TiVec
    pub vehicles: Vec<Vehicle>,
    pub vehicle_ids: IDMapping<VehicleName, VehicleID>,
    pub gtfs: GTFS,
    pub journeys: Vec<Journey>,

    // TODO This is derived from other things, and may outright replace it at some point
    // Sorted by arrival time
    pub boardings: Vec<BoardingEvent>,

    // If we've loaded journey and vehicle data, this is the one day covered. If not, it's an
    // arbitrary date covered by some GTFS service.
    pub main_date: NaiveDate,
}

#[derive(Serialize, Deserialize)]
pub struct Vehicle {
    pub id: VehicleID,
    pub original_id: VehicleName,
    pub trajectory: Trajectory,
    // Calculated
    pub timetable: Timetable<gtfs::TripID>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VehicleName(pub(crate) String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VehicleID(pub usize);

impl gtfs::CheapID for VehicleID {
    fn new(x: usize) -> Self {
        Self(x)
    }
}

impl Model {
    /// Returns a daily model for everything in the input .zip
    pub fn import_zip_bytes(bytes: Vec<u8>, timer: &mut Timer) -> Result<Vec<Self>> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;

        timer.start("loading GTFS");
        let (gtfs, gps_bounds) = GTFS::load_from_dir(&mut archive)?;
        timer.stop("loading GTFS");

        let avl_files = find_all_files(&archive, "avl/avl_");
        let bil_files = find_all_files(&archive, "bil/bil_");
        let daily_input_files = find_common_files(avl_files, bil_files);

        let mut output_models = Vec::new();

        for (date, avl_path, bil_path) in daily_input_files {
            timer.start(format!("import daily data for {date}"));
            let mut vehicles = Vec::new();
            let mut vehicle_ids = IDMapping::new();

            timer.start("loading AVL");
            for (original_id, trajectory) in
                avl::load_trajectories(archive.by_name(&avl_path)?, &gps_bounds, date)?
            {
                let id = vehicle_ids.insert_new(original_id.clone())?;
                vehicles.push(Vehicle {
                    id,
                    original_id,
                    trajectory,
                    timetable: Timetable::new(),
                });
            }
            timer.stop("loading AVL");

            timer.start("loading BIL");
            let journeys =
                ticketing::load_journeys(archive.by_name(&bil_path)?, &gps_bounds, date)?;
            timer.stop("loading BIL");

            let mut model = Self {
                bounds: gps_bounds.to_bounds(),
                gps_bounds: gps_bounds.clone(),
                vehicles,
                vehicle_ids,
                gtfs: gtfs.clone(),
                journeys,
                boardings: Vec::new(),
                main_date: date,
            };
            assemble::populate_boarding(&mut model, timer)?;

            output_models.push(model);
            timer.stop(format!("import daily data for {date}"));
        }

        if output_models.is_empty() {
            // An empty GTFS-only model
            let main_date = gtfs.calendar.services.values().next().unwrap().start_date;
            output_models.push(Self {
                bounds: gps_bounds.to_bounds(),
                gps_bounds,
                vehicles: Vec::new(),
                vehicle_ids: IDMapping::new(),
                gtfs,
                journeys: Vec::new(),
                boardings: Vec::new(),
                main_date,
            });
        }

        Ok(output_models)
    }

    pub fn empty() -> Self {
        Self {
            // Avoid crashing the UI with empty bounds
            bounds: Bounds::from(&[Pt2D::zero(), Pt2D::new(1.0, 1.0)]),
            gps_bounds: GPSBounds::new(),
            vehicles: Vec::new(),
            vehicle_ids: IDMapping::new(),
            gtfs: GTFS::empty(),
            journeys: Vec::new(),
            boardings: Vec::new(),
            main_date: NaiveDate::from_ymd(2020, 1, 1),
        }
    }

    pub fn lookup_vehicle(&self, name: &VehicleName) -> Option<&Vehicle> {
        let id = self.vehicle_ids.lookup(name).ok()?;
        Some(&self.vehicles[id.0])
    }
}

fn find_all_files(
    archive: &zip::ZipArchive<std::io::Cursor<Vec<u8>>>,
    prefix: &str,
) -> BTreeMap<NaiveDate, String> {
    let mut results = BTreeMap::new();
    for file_name in archive.file_names() {
        if let Some(x) = file_name.strip_prefix(prefix) {
            if let Some(x) = x.strip_suffix(".csv") {
                if let Ok(date) = NaiveDate::parse_from_str(x, "%Y-%m-%d") {
                    results.insert(date, file_name.to_string());
                }
            }
        }
    }
    results
}

fn find_common_files(
    avl_files: BTreeMap<NaiveDate, String>,
    mut bil_files: BTreeMap<NaiveDate, String>,
) -> Vec<(NaiveDate, String, String)> {
    let mut results = Vec::new();
    for (date, avl) in avl_files {
        if let Some(bil) = bil_files.remove(&date) {
            results.push((date, avl, bil));
        } else {
            warn!("We have {avl} but not the BIL equivalent");
        }
    }
    for (_, bil) in bil_files {
        warn!("We have {bil} but not the AVL equivalent");
    }
    results
}
