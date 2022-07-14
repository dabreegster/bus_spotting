use std::collections::BTreeMap;

use anyhow::Result;
use chrono::{NaiveDate, NaiveDateTime, Timelike};
use geom::{Duration, GPSBounds, LonLat, Pt2D, Time};
use serde::Deserialize;

use crate::{Trajectory, VehicleName};

pub fn load_trajectories<R: std::io::Read>(
    reader: R,
    gps_bounds: &GPSBounds,
    expected_date: NaiveDate,
) -> Result<BTreeMap<VehicleName, Trajectory>> {
    // Read raw data
    let mut data_per_vehicle: BTreeMap<VehicleName, Vec<(LonLat, Time)>> = BTreeMap::new();
    for rec in csv::Reader::from_reader(reader).deserialize() {
        let rec: AVL = rec?;

        let datetime = NaiveDateTime::parse_from_str(&rec.datetime, "%Y-%m-%d %H:%M:%S")?;
        if datetime.date() != expected_date {
            bail!(
                "An AVL file for {} contains a record for {}",
                expected_date,
                datetime
            );
        }

        let time = datetime.time();
        let time = Time::START_OF_DAY
            + Duration::hours(time.hour() as usize)
            + Duration::minutes(time.minute() as usize)
            + Duration::seconds(time.second() as f64);

        let pos = LonLat::new(rec.longitude, rec.latitude);

        data_per_vehicle
            .entry(rec.vehicle_name)
            .or_insert_with(Vec::new)
            .push((pos, time));
    }

    // Calculate trajectories
    let mut results = BTreeMap::new();
    for (vehicle_name, raw_pts) in data_per_vehicle {
        let mut points: Vec<(Pt2D, Time)> = Vec::new();
        for (gps, time) in raw_pts {
            points.push((gps.to_pt(&gps_bounds), time));
        }
        results.insert(vehicle_name, Trajectory::new(points)?);
    }
    Ok(results)
}

#[derive(Deserialize)]
struct AVL {
    #[serde(rename = "CODVEICULO")]
    vehicle_name: VehicleName,
    #[serde(rename = "DATAHORACOORD")]
    datetime: String,
    #[serde(rename = "LATITUDE")]
    latitude: f64,
    #[serde(rename = "LONGITUDE")]
    longitude: f64,
}
