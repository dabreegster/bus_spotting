use std::collections::BTreeMap;

use anyhow::Result;
use geom::PolyLine;
use serde::{Deserialize, Serialize};

use super::{ServiceID, ShapeID, StopID, Trip, GTFS};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RouteID(String);

#[derive(Serialize, Deserialize)]
pub struct Route {
    pub route_id: RouteID,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub description: Option<String>,

    pub variants: Vec<RouteVariant>,
}

#[derive(Serialize, Deserialize)]
pub struct RouteVariant {
    pub route_id: RouteID,
    pub variant_id: RouteVariantID,
    // Sorted by time
    pub trips: Vec<Trip>,
    pub headsign: Option<String>,
    pub service_id: ServiceID,
    pub shape_id: ShapeID,
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

    pub fn stops(&self) -> Vec<StopID> {
        self.trips[0]
            .stop_times
            .iter()
            .map(|st| st.stop_id)
            .collect()
    }

    /// If GTFS has a shape, use that. Otherwise calculated from straight lines between stops
    pub fn polyline(&self, gtfs: &GTFS) -> Result<PolyLine> {
        if let Some(pl) = gtfs.shapes.get(&self.shape_id) {
            return Ok(pl.clone());
        }

        let mut pts = Vec::new();
        for stop_time in &self.trips[0].stop_times {
            pts.push(gtfs.stops[&stop_time.stop_id].pos);
        }
        PolyLine::new(pts)
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
