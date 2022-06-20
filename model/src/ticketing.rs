use std::collections::BTreeMap;

use anyhow::Result;
use chrono::{NaiveDate, NaiveDateTime, Timelike};
use geom::{Duration, GPSBounds, LonLat, Pt2D, Time};
use serde::{Deserialize, Serialize};

use crate::{Model, Trajectory, VehicleName};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CardID(String);

// Just an index into journeys for now
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct JourneyID(pub(crate) usize);

#[derive(Serialize, Deserialize)]
pub struct Journey {
    pub card_id: CardID,
    pub legs: Vec<JourneyLeg>,
}

#[derive(Serialize, Deserialize)]
pub struct JourneyLeg {
    // Sometime after boarding
    pub time: Time,
    pub pos: Pt2D,
    pub route_short_name: String,
    pub vehicle_name: VehicleName,
}

pub fn load<R: std::io::Read>(
    reader: R,
    gps_bounds: &GPSBounds,
) -> Result<(Vec<Journey>, NaiveDate)> {
    let mut per_card: BTreeMap<CardID, Vec<JourneyLeg>> = BTreeMap::new();
    let mut main_date = None;

    for rec in csv::Reader::from_reader(reader).deserialize() {
        let rec: BIL = rec?;

        let datetime = NaiveDateTime::parse_from_str(&rec.datetime, "%d/%m/%Y %H:%M:%S")?;
        // Assume the input is sorted by time. Entries at the very end may leak over into the next
        // day. Until we handle multi-day fully, just add 24 hours to the time.
        if main_date.is_none() {
            main_date = Some(datetime.date());
        }
        let next_day = if Some(datetime.date()) == main_date {
            Duration::ZERO
        } else {
            // Assume next day
            Duration::hours(24)
        };

        let time = datetime.time();
        let time = Time::START_OF_DAY
            + Duration::hours(time.hour() as usize)
            + Duration::minutes(time.minute() as usize)
            + Duration::seconds(time.second() as f64)
            + next_day;

        per_card
            .entry(rec.card_id)
            .or_insert_with(Vec::new)
            .push(JourneyLeg {
                time,
                pos: LonLat::new(rec.longitude, rec.latitude).to_pt(gps_bounds),
                route_short_name: rec.route_short_name,
                vehicle_name: rec.vehicle_name,
            });
    }

    Ok((
        per_card.into_iter().flat_map(split_into_journeys).collect(),
        main_date.unwrap(),
    ))
}

#[derive(Deserialize)]
struct BIL {
    #[serde(rename = "DATA")]
    datetime: String,
    #[serde(rename = "CODVEICULO")]
    vehicle_name: VehicleName,
    #[serde(rename = "CODLINHA")]
    route_short_name: String,
    #[serde(rename = "NUMEROCARTAO")]
    card_id: CardID,
    #[serde(rename = "LATITUDE")]
    latitude: f64,
    #[serde(rename = "LONGITUDE")]
    longitude: f64,
}

// A passenger can board up to four buses in a two-hour window
fn split_into_journeys((card_id, mut legs): (CardID, Vec<JourneyLeg>)) -> Vec<Journey> {
    legs.sort_by_key(|leg| leg.time);

    let mut journeys = Vec::new();
    let mut current_legs = vec![legs.remove(0)];
    for leg in legs {
        // TODO How's the two-hour window defined -- starting from the first event, or the most
        // recent? (Can somebody ride for a total of 10 hours, with 90 minutes between each
        // ticket?)
        if current_legs.len() < 4 && leg.time - current_legs[0].time < Duration::hours(2) {
            current_legs.push(leg);
        } else {
            journeys.push(Journey {
                card_id: card_id.clone(),
                legs: std::mem::take(&mut current_legs),
            });
            current_legs.push(leg);
        }
    }
    journeys.push(Journey {
        card_id: card_id.clone(),
        legs: current_legs,
    });
    journeys
}

impl Model {
    pub fn set_alt_trajectories_from_ticketing(&mut self) {
        let mut pts_per_vehicle: BTreeMap<VehicleName, Vec<(Pt2D, Time)>> = BTreeMap::new();
        for journey in &self.journeys {
            for leg in &journey.legs {
                pts_per_vehicle
                    .entry(leg.vehicle_name.clone())
                    .or_insert_with(Vec::new)
                    .push((leg.pos, leg.time));
            }
        }
        let mut modified = 0;
        for (vehicle_name, mut pts) in pts_per_vehicle {
            pts.sort_by_key(|(_, t)| *t);
            match Trajectory::new(pts) {
                Ok(trajectory) => {
                    match self
                        .vehicles
                        .iter_mut()
                        .find(|v| v.original_id == vehicle_name)
                    {
                        Some(vehicle) => {
                            modified += 1;
                            vehicle.alt_trajectory = Some(trajectory);
                        }
                        None => {
                            warn!(
                                "Ticketing data refers to unknown vehicle {:?}",
                                vehicle_name
                            );
                        }
                    }
                }
                Err(err) => {
                    warn!(
                        "Couldn't make trajectory from ticketing for {:?}: {}",
                        vehicle_name, err
                    );
                }
            }
        }
        info!(
            "Overrode trajectories for {modified} / {} vehicles",
            self.vehicles.len()
        );
    }
}
