use anyhow::Result;
use geom::{Distance, PolyLine, Time};

use crate::{IDMapping, Model, Timetable, Trajectory, Vehicle, VehicleName};
use gtfs::{DateFilter, RouteVariant, RouteVariantID, TripID, VariantFilter};

impl Model {
    // Turn each trip of a variant into a trajectory, using the stop times.
    //
    // Interesting to replay the "expected state", but hard to compare to real data. As delays
    // increase or the number of trips for a variant differ, figuring out what a bus was supposed
    // to be doing...
    pub fn trajectories_for_variant(
        &self,
        variant: RouteVariantID,
    ) -> Result<Vec<(TripID, Trajectory)>> {
        let variant = self.gtfs.variant(variant);
        let shape_pl = &self.gtfs.shapes[&variant.shape_id];

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

    pub fn replace_vehicles_with_gtfs(&mut self) {
        self.vehicles.clear();
        self.vehicle_ids = IDMapping::new();
        self.journeys.clear();
        self.boardings.clear();

        // Only for the main_date
        let filter = VariantFilter {
            date_filter: DateFilter::SingleDay(self.main_date),
            minimum_trips_per_day: 0,
        };

        let mut all_trajectories = Vec::new();
        for id in &self.gtfs.variants_matching_filter(&filter) {
            match self.trajectories_for_variant(*id) {
                Ok(list) => {
                    all_trajectories.extend(list);
                }
                Err(err) => {
                    error!("{:?} didn't work: {}", id, err);
                }
            }
        }

        // One vehicle per trip
        for (trip, trajectory) in all_trajectories {
            let original_id = VehicleName(format!("{:?}", trip));
            let id = self.vehicle_ids.insert_new(original_id.clone()).unwrap();
            self.vehicles.push(Vehicle {
                id,
                original_id,
                trajectory,
                timetable: Timetable::new(),
            });
        }
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
