use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{Trip, TripID};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RouteID(String);

#[derive(Serialize, Deserialize)]
pub struct Route {
    pub route_id: RouteID,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub description: Option<String>,

    // TODO Once we have our own cheap trip IDs, consider sorting by time
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
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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
