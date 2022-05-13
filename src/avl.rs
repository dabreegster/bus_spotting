use std::collections::BTreeMap;
// TODO fs_err
use std::fs::File;

use anyhow::Result;
use chrono::{NaiveDateTime, Timelike};
use geom::{Duration, GPSBounds, LonLat, Pt2D, Time};
use serde::Deserialize;

use crate::{Trajectory, VehicleName};

pub fn load(path: &str) -> Result<(GPSBounds, BTreeMap<VehicleName, Trajectory>)> {
    // Read raw data
    let mut data_per_vehicle: BTreeMap<VehicleName, Vec<(LonLat, Time)>> = BTreeMap::new();
    for rec in csv::Reader::from_reader(File::open(path)?).deserialize() {
        let rec: AVL = rec?;

        let datetime = NaiveDateTime::parse_from_str(&rec.datetime, "%Y-%m-%d %H:%M:%S")?;
        // Ignore the date
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

    // Calculate bounds from this one file. We'll probably do this from GTFS instead.
    let mut gps_bounds = GPSBounds::new();
    for points in data_per_vehicle.values() {
        for (pos, _) in points {
            gps_bounds.update(*pos);
        }
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
    Ok((gps_bounds, results))
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
