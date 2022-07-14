use std::collections::BTreeMap;

use anyhow::Result;
use geom::{Distance, GPSBounds, LonLat, PolyLine, Pt2D};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ShapeID(String);

pub fn load<R: std::io::Read>(
    reader: R,
    gps_bounds: &GPSBounds,
) -> Result<BTreeMap<ShapeID, PolyLine>> {
    let mut pts_per_shape: BTreeMap<ShapeID, Vec<(usize, Pt2D)>> = BTreeMap::new();
    for rec in csv::Reader::from_reader(reader).deserialize() {
        let rec: Record = rec?;
        let pos = LonLat::new(rec.shape_pt_lon, rec.shape_pt_lat).to_pt(gps_bounds);
        pts_per_shape
            .entry(rec.shape_id)
            .or_insert_with(Vec::new)
            .push((rec.shape_pt_sequence, pos));
    }

    // Sort by shape_pt_sequence, in case the file isn't in order
    let mut results = BTreeMap::new();
    for (shape_id, mut pts) in pts_per_shape {
        pts.sort_by_key(|(seq, _)| *seq);
        let pts: Vec<Pt2D> = pts.into_iter().map(|(_, pt)| pt).collect();
        let pts = Pt2D::approx_dedupe(pts, Distance::meters(1.0));
        // TODO The shape can double back on itself. Should we split into multiple segments instead
        // of doing this?
        let pl = PolyLine::unchecked_new(pts);
        results.insert(shape_id, pl);
    }
    Ok(results)
}

#[derive(Deserialize)]
struct Record {
    shape_id: ShapeID,
    shape_pt_lat: f64,
    shape_pt_lon: f64,
    shape_pt_sequence: usize,
}
