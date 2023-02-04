use std::collections::BTreeMap;

use anyhow::Result;
use geom::{GPSBounds, PolyLine};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use super::{ServiceID, ShapeID, StopID, Trip, GTFS};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RouteID(String);

#[derive(Clone, Serialize, Deserialize)]
pub struct Route {
    pub route_id: RouteID,
    pub route_type: RouteType,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub description: Option<String>,

    pub variants: Vec<RouteVariant>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum RouteType {
    Tram = 0,
    Subway = 1,
    Rail = 2,
    Bus = 3,
    Ferry = 4,
    CableTram = 5,
    AerialLift = 6,
    Furnicular = 7,
    Trolleybus = 11,
    Monorail = 12,
}

impl RouteType {
    pub fn all() -> Vec<Self> {
        use RouteType::*;
        vec![
            Tram, Subway, Rail, Bus, Ferry, CableTram, AerialLift, Furnicular, Trolleybus, Monorail,
        ]
    }
}

#[derive(Clone, Serialize, Deserialize)]
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
        let name = self
            .short_name
            .as_ref()
            .or(self.long_name.as_ref())
            .or(self.description.as_ref())
            .map(|x| x.to_string())
            .unwrap_or_else(|| format!("{:?}", self.route_id));
        format!("{name} ({:?})", self.route_type)
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

    /// If GTFS has an original shape, use that. Otherwise calculated from straight lines between stops.
    pub fn polyline(&self, gtfs: &GTFS) -> Result<PolyLine> {
        if let Some(pl) = gtfs.shapes.get(&self.shape_id) {
            return Ok(pl.clone());
        }

        let mut pts = Vec::new();
        for stop_id in self.stops() {
            pts.push(gtfs.stops[&stop_id].pos);
        }
        PolyLine::new(pts)
    }

    pub fn export_to_geojson(
        &self,
        path: String,
        gtfs: &GTFS,
        gps_bounds: &GPSBounds,
    ) -> Result<()> {
        use geojson::{Feature, FeatureCollection, GeoJson};

        let mut features = Vec::new();

        let mut feature = Feature {
            bbox: None,
            geometry: Some(self.polyline(gtfs)?.to_geojson(Some(gps_bounds))),
            id: None,
            properties: None,
            foreign_members: None,
        };
        feature.set_property("type", "route");
        features.push(feature);

        for (idx, stop) in self.stops().into_iter().enumerate() {
            let pos = gtfs.stops[&stop].pos.to_gps(gps_bounds);
            let mut feature = Feature {
                bbox: None,
                geometry: Some(geojson::Geometry::new(geojson::Value::Point(vec![
                    pos.x(),
                    pos.y(),
                ]))),
                id: None,
                properties: None,
                foreign_members: None,
            };
            feature.set_property("type", "stop");
            feature.set_property("stop_sequence", idx + 1);
            features.push(feature);
        }

        let gj = GeoJson::FeatureCollection(FeatureCollection {
            features,
            bbox: None,
            foreign_members: None,
        });
        std::fs::write(path, serde_json::to_string_pretty(&gj)?)?;
        Ok(())
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
                route_type: rec.route_type,
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
    route_type: RouteType,
    route_short_name: Option<String>,
    route_long_name: Option<String>,
    route_desc: Option<String>,
    // TODO Assuming route_type = 3
}
