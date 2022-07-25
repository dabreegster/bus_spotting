use abstutil::Counter;
use chrono::NaiveDate;
use geom::{Bounds, GPSBounds, Pt2D};
use serde::{Deserialize, Serialize};

use gtfs::{StopID, GTFS};

use crate::{BoardingEvent, DailyModel};

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
    pub fn new_from_daily_models(models: &Vec<DailyModel>) -> Self {
        let mut output = Self {
            bounds: models[0].bounds.clone(),
            gps_bounds: models[0].gps_bounds.clone(),
            gtfs: models[0].gtfs.clone(),

            boardings_per_day: Vec::new(),
        };

        for model in models {
            output
                .boardings_per_day
                .push((model.date, model.boardings.clone()));
        }
        output.boardings_per_day.sort_by_key(|(d, _)| *d);

        output
    }

    pub fn empty() -> Self {
        Self {
            // Avoid crashing the UI with empty bounds
            bounds: Bounds::from(&[Pt2D::zero(), Pt2D::new(1.0, 1.0)]),
            gps_bounds: GPSBounds::new(),
            gtfs: GTFS::empty(),
            boardings_per_day: Vec::new(),
        }
    }

    pub fn count_boardings_by_stop(&self) -> Counter<StopID> {
        let mut cnt = Counter::new();
        for (_, events) in &self.boardings_per_day {
            for ev in events {
                cnt.add(ev.stop, ev.new_riders.len() + ev.transfers.len());
            }
        }
        cnt
    }
}
