use std::collections::BTreeMap;

use anyhow::Result;
use geom::Time;
use serde::{Deserialize, Serialize};

use super::{orig, IDMapping, StopID, TripID};

#[derive(Serialize, Deserialize)]
pub struct StopTime {
    pub arrival_time: Time,
    pub departure_time: Time,
    pub stop_id: StopID,
}

pub fn load<R: std::io::Read>(
    reader: R,
    stop_ids: &IDMapping,
) -> Result<BTreeMap<TripID, Vec<StopTime>>> {
    let mut stop_times = BTreeMap::new();
    for rec in csv::Reader::from_reader(reader).deserialize() {
        let rec: Record = rec?;
        let arrival_time = Time::parse(&rec.arrival_time)?;
        let departure_time = Time::parse(&rec.departure_time)?;
        if arrival_time > departure_time {
            bail!("Arrival time {arrival_time} is > departure time {departure_time}");
        }
        stop_times
            .entry(rec.trip_id)
            .or_insert_with(Vec::new)
            .push((
                rec.stop_sequence,
                StopTime {
                    arrival_time,
                    departure_time,
                    stop_id: stop_ids.lookup(&rec.stop_id)?,
                },
            ));
    }

    // Sort by stop_sequence, in case the file isn't in order
    let mut results = BTreeMap::new();
    for (trip_id, mut stops) in stop_times {
        stops.sort_by_key(|(seq, _)| *seq);
        results.insert(
            trip_id,
            stops.into_iter().map(|(_, stop_time)| stop_time).collect(),
        );
    }
    Ok(results)
}

#[derive(Deserialize)]
struct Record {
    trip_id: TripID,
    arrival_time: String,
    departure_time: String,
    stop_id: orig::StopID,
    stop_sequence: usize,
}
