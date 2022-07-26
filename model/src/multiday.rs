use abstutil::Counter;
use anyhow::Result;
use chrono::NaiveDate;
use geom::{Bounds, GPSBounds, Pt2D, Time};
use serde::{Deserialize, Serialize};

use gtfs::{orig, IDMapping, RouteID, RouteVariantID, StopID, GTFS};

use crate::{BoardingEvent, DailyModel, VehicleID, VehicleName};

/// Summarizes bus data for many days.
#[derive(Serialize, Deserialize)]
pub struct MultidayModel {
    pub bounds: Bounds,
    pub gps_bounds: GPSBounds,
    pub gtfs: GTFS,

    // The list of days is sorted. Boardings per day are sorted by arrival time
    pub boardings_per_day: Vec<(NaiveDate, Vec<BoardingEvent>)>,
    pub vehicle_ids: IDMapping<VehicleName, VehicleID>,
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
            vehicle_ids: IDMapping::new(),
        };

        for model in models {
            let mut events = Vec::new();
            for ev in &model.boardings {
                // Vehicle ID assignment may change each day, so calculate again from the original
                // VehicleName
                let vehicle_name = &model.vehicles[ev.vehicle.0].original_id;
                let vehicle_id = output.vehicle_ids.insert_idempotent(vehicle_name);
                let mut ev = ev.clone();
                ev.vehicle = vehicle_id;
                events.push(ev);
            }

            output.boardings_per_day.push((model.date, events));
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
            vehicle_ids: IDMapping::new(),
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

    pub fn export_to_csv(&self) -> Result<String> {
        let mut vehicle_ids: Vec<VehicleName> =
            std::iter::repeat_with(|| VehicleName(String::new()))
                .take(self.vehicle_ids.borrow().len())
                .collect();
        for (orig, cheap) in self.vehicle_ids.borrow() {
            vehicle_ids[cheap.0] = orig.clone();
        }

        let mut out = Vec::new();
        {
            let mut writer = csv::Writer::from_writer(&mut out);
            for (date, events) in &self.boardings_per_day {
                for ev in events {
                    let route = self.gtfs.parent_of_variant(ev.variant);
                    let variant = self.gtfs.variant(ev.variant);
                    let trip = variant.trips.iter().find(|t| t.id == ev.trip).unwrap();

                    writer.serialize(ExportBoardingRow {
                        date: *date,
                        vehicle: vehicle_ids[ev.vehicle.0].clone(),
                        route_id: route.route_id.clone(),
                        route_variant: ev.variant,
                        trip: trip.orig_id.clone(),
                        stop: self.gtfs.stops[&ev.stop].orig_id.clone(),
                        arrival_time: ev.arrival_time,
                        departure_time: ev.departure_time,
                        new_riders: ev.new_riders.len(),
                        transfers: ev.transfers.len(),
                    })?;
                }
            }
            writer.flush()?;
        }
        let out = String::from_utf8(out)?;
        Ok(out)
    }
}

#[derive(Serialize)]
struct ExportBoardingRow {
    date: NaiveDate,
    vehicle: VehicleName,
    route_id: RouteID,
    route_variant: RouteVariantID,
    trip: orig::TripID,
    stop: orig::StopID,
    arrival_time: Time,
    departure_time: Time,
    new_riders: usize,
    transfers: usize,
}
