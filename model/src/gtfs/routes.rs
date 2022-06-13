use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{ServiceID, StopID, Trip, TripID, GTFS};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RouteID(String);

#[derive(Serialize, Deserialize)]
pub struct Route {
    pub route_id: RouteID,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub description: Option<String>,

    // TODO Once we have our own cheap trip IDs, consider sorting by time
    // TODO Store these in the variants directly
    pub trips: BTreeMap<TripID, Trip>,
    pub variants: Vec<RouteVariant>,
}

#[derive(Serialize, Deserialize)]
pub struct RouteVariant {
    pub route_id: RouteID,
    pub variant_id: RouteVariantID,
    // Sorted by time
    pub trips: Vec<TripID>,
    pub headsign: Option<String>,
    pub service_id: ServiceID,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RouteVariantID(pub usize);

impl Route {
    pub fn describe(&self) -> String {
        for x in [&self.short_name, &self.long_name, &self.description] {
            if let Some(x) = x {
                return x.to_string();
            }
        }
        format!("{:?}", self.route_id)
    }
}

impl RouteVariant {
    pub fn describe(&self, gtfs: &GTFS) -> String {
        let headsign = match self.headsign {
            Some(ref x) => format!("{:?} ({x})", self.variant_id),
            None => format!("{:?}", self.variant_id),
        };
        format!(
            " {} {} - {}, {} trips",
            gtfs.routes[&self.route_id].describe(),
            headsign,
            gtfs.calendar.services[&self.service_id]
                .days_of_week
                .describe(),
            self.trips.len()
        )
    }

    pub fn stops(&self, gtfs: &GTFS) -> Vec<StopID> {
        let mut stops = Vec::new();
        for st in &gtfs.routes[&self.route_id].trips[&self.trips[0]].stop_times {
            stops.push(st.stop_id.clone());
        }
        stops
    }
}

pub fn load<R: std::io::Read>(reader: R) -> Result<BTreeMap<RouteID, Route>> {
    let mut routes = BTreeMap::new();
    for rec in csv::Reader::from_reader(reader).deserialize() {
        let rec: Record = rec?;
        if routes.contains_key(&rec.route_id) {
            bail!("Duplicate {:?}", rec.route_id);
        }
        routes.insert(
            rec.route_id.clone(),
            Route {
                route_id: rec.route_id,
                short_name: rec.route_short_name,
                long_name: rec.route_long_name,
                description: rec.route_desc,

                trips: BTreeMap::new(),
                variants: Vec::new(),
            },
        );
    }
    Ok(routes)
}

#[derive(Deserialize)]
struct Record {
    route_id: RouteID,
    route_short_name: Option<String>,
    route_long_name: Option<String>,
    route_desc: Option<String>,
    // TODO Assuming route_type = 3
}
