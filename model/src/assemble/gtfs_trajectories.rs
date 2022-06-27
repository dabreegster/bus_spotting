use anyhow::Result;
use geom::{Distance, PolyLine, Time};

use crate::gtfs::{RouteVariant, RouteVariantID, TripID};
use crate::{Model, Trajectory};

impl Model {
    pub fn trajectories_for_variant(
        &self,
        variant: RouteVariantID,
    ) -> Result<Vec<(TripID, Trajectory)>> {
        let variant = self.gtfs.variant(variant);
        let shape_pl = &self.gtfs.shapes[&variant.shape_id];

        info!(
            "trajectories_for_variant {:?}, there are {} trips",
            variant.variant_id,
            variant.trips.len()
        );

        let split_shape = split_shape_by_stops(self, shape_pl, variant)?;

        let mut trajectories = Vec::new();
        for trip in &variant.trips {
            // TODO Use both times when they differ?
            let times: Vec<Time> = trip.stop_times.iter().map(|st| st.arrival_time).collect();
            trajectories.push((
                trip.id,
                Trajectory::from_pieces_with_times(&split_shape, times)?,
            ));
        }
        Ok(trajectories)
    }
}

fn split_shape_by_stops(
    model: &Model,
    shape_pl: &PolyLine,
    variant: &RouteVariant,
) -> Result<Vec<PolyLine>> {
    // Snap the stops to distances along the shape
    let mut stop_distances = Vec::new();
    for stop_id in variant.stops() {
        let projected_pt = shape_pl.project_pt(model.gtfs.stops[&stop_id].pos);
        if let Some((dist, _)) = shape_pl.dist_along_of_point(projected_pt) {
            stop_distances.push(dist);
        } else {
            bail!("Couldn't find {:?} along shape", stop_id);
        }
    }

    // Check the distance along is increasing
    for pair in stop_distances.windows(2) {
        if pair[0] >= pair[1] {
            bail!(
                "Stop distances along shape out-of-order: {} then {}",
                pair[0],
                pair[1]
            );
        }
    }

    // TODO Is the first stop at 0? Is the last at the length of the pl?

    chop_polyline(shape_pl, stop_distances)
}

fn chop_polyline(pl: &PolyLine, distances: Vec<Distance>) -> Result<Vec<PolyLine>> {
    let mut results = Vec::new();
    for pair in distances.windows(2) {
        results.push(pl.maybe_exact_slice(pair[0], pair[1])?);
    }
    Ok(results)
}
