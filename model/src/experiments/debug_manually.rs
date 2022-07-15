use anyhow::Result;
use geom::UnitFmt;

use crate::{DailyModel, Trajectory, VehicleID};

impl DailyModel {
    // These might be easy to inspect manually.
    //
    // But they mostly seem to be very short routes around a campus
    pub fn vehicles_with_few_stops(&self) -> Result<()> {
        for (vehicle, variants) in self.vehicles_to_possible_routes()? {
            for v in variants {
                let variant = self.gtfs.variant(v);
                if variant.stops().len() < 15 {
                    let shape = &self.gtfs.shapes[&variant.shape_id];
                    println!(
                        "{:?} possibly serves a simple route {:?} with length {}",
                        vehicle,
                        v,
                        shape.length().to_string(&UnitFmt::metric())
                    );
                }
            }
        }
        Ok(())
    }

    // Use each possible trip as a trajectory. Many results.
    pub fn possible_trip_trajectories_for_vehicle(
        &self,
        id: VehicleID,
    ) -> Result<Vec<(String, Trajectory)>> {
        let variants = self.vehicle_to_possible_routes(id);

        let mut result = Vec::new();
        for variant in variants {
            let variant_description = self.gtfs.variant(variant).describe(&self.gtfs);
            for (trip, trajectory) in self.trajectories_for_variant(variant)? {
                result.push((format!("{:?} of {}", trip, variant_description), trajectory));
            }
        }
        Ok(result)
    }

    // Use each route's shape as a trajectory, making up nonsense times. Few results.
    pub fn possible_route_trajectories_for_vehicle(
        &self,
        id: VehicleID,
    ) -> Result<Vec<(String, Trajectory)>> {
        let variants = self.vehicle_to_possible_routes(id);

        // We could group by shape, but the UI actually cares about disambiguating, so don't bother
        let mut result = Vec::new();
        for variant in variants {
            let variant = self.gtfs.variant(variant);
            let pl = &self.gtfs.shapes[&variant.shape_id];
            result.push((variant.describe(&self.gtfs), Trajectory::from_polyline(pl)));
        }
        Ok(result)
    }
}
