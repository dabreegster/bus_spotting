use std::collections::BTreeMap;

use anyhow::Result;
use fs_err::File;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RouteID(String);

#[derive(Serialize, Deserialize)]
pub struct Route {
    pub route_id: RouteID,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub description: Option<String>,
}

pub fn load(path: String) -> Result<BTreeMap<RouteID, Route>> {
    let mut routes = BTreeMap::new();
    for rec in csv::Reader::from_reader(File::open(path)?).deserialize() {
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
