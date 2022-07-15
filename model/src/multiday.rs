use chrono::NaiveDate;
use geom::{Bounds, GPSBounds};
use serde::{Deserialize, Serialize};

use gtfs::GTFS;

use crate::{BoardingEvent, Model};

/// Summarizes bus data for many days.
#[derive(Serialize, Deserialize)]
pub struct MultidayModel {
    pub bounds: Bounds,
    pub gps_bounds: GPSBounds,
    pub gtfs: GTFS,

    // The list of days is sorted. Boardings per day are sorted by arrival time
    pub boardings_per_day: Vec<(NaiveDate, Vec<BoardingEvent>)>,
    // TODO Include journeys too, probably. But re-express on top of the BoardingEvents / don't
    // store the route name and vehicle again.
}

impl MultidayModel {
    // Assumes at least 1 input and that the inputs all have the same bounds / GTFS data
    pub fn new_from_daily_models(models: &Vec<Model>) -> Self {
        let mut output = Self {
            bounds: models[0].bounds.clone(),
            gps_bounds: models[0].gps_bounds.clone(),
            gtfs: models[0].gtfs.clone(),

            boardings_per_day: Vec::new(),
        };

        for model in models {
            output
                .boardings_per_day
                .push((model.main_date, model.boardings.clone()));
        }
        output.boardings_per_day.sort_by_key(|(d, _)| *d);

        output
    }
}
