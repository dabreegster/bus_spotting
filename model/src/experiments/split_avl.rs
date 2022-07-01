use anyhow::Result;
use geom::Distance;

use crate::gtfs::RouteVariantID;
use crate::{Model, Trajectory, VehicleID};

impl Model {
    // Find all times the AVL is close to endpoints of a possible variant's shape. Use those to clip
    // into multiple pieces, just view the result
    //
    // This idea could be worth finishing, but is probably subsumed by matching to stops
    pub fn split_avl_by_route_shape(&self, vehicle: VehicleID) -> Result<()> {
        let list = self.vehicle_to_possible_routes(vehicle);
        if !list.is_empty() {
            // TODO Just do the first one
            self.segment_avl_by_endpoints(vehicle, list[0]);
        }
        Ok(())
    }

    fn segment_avl_by_endpoints(
        &self,
        vehicle: VehicleID,
        variant: RouteVariantID,
    ) -> Vec<Trajectory> {
        let variant = self.gtfs.variant(variant);
        let shape_pl = &self.gtfs.shapes[&variant.shape_id];

        let vehicle_trajectory = &self.vehicles[vehicle.0].trajectory;

        let threshold = Distance::meters(10.0);
        let times_near_start = vehicle_trajectory.times_near_pos(shape_pl.first_pt(), threshold);
        let times_near_end = vehicle_trajectory.times_near_pos(shape_pl.last_pt(), threshold);

        if true {
            println!("does {:?} match {:?}?", vehicle, variant.variant_id);
            println!("near start at:");
            for (t, _) in &times_near_start {
                println!("- {t}");
            }
            println!("near end at:");
            for (t, _) in &times_near_end {
                println!("- {t}");
            }
        }

        // Attempt to match up the times
        let mut intervals = Vec::new();
        for ((t1, _), (t2, _)) in times_near_start.into_iter().zip(times_near_end.into_iter()) {
            if t1 < t2 {
                if intervals
                    .last()
                    .map(|(_, prev_t2)| t1 > *prev_t2)
                    .unwrap_or(true)
                {
                    intervals.push((t1, t2));
                    println!("... so interval from {t1} to {t2}");
                    continue;
                }
            }
            println!("??? what happened from {t1} to {t2}");
        }

        // TODO Now clip_to_time for the good matches? Or if there's unexplainable bits, don't
        // consider this a good match?

        Vec::new()
    }
}
