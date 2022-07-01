use std::collections::BTreeMap;

use geom::{Pt2D, Time};

use crate::{Model, Trajectory, VehicleName};

impl Model {
    // Not helpful to see. When there's nobody boarding at a stop, it looks like the vehicle cuts
    // in a straight line across many stops. The resulting path is very noisy.
    pub fn set_alt_trajectories_from_ticketing(&mut self) {
        let mut pts_per_vehicle: BTreeMap<VehicleName, Vec<(Pt2D, Time)>> = BTreeMap::new();
        for journey in &self.journeys {
            for leg in &journey.legs {
                pts_per_vehicle
                    .entry(leg.vehicle_name.clone())
                    .or_insert_with(Vec::new)
                    .push((leg.pos, leg.time));
            }
        }
        let mut modified = 0;
        for (vehicle_name, mut pts) in pts_per_vehicle {
            pts.sort_by_key(|(_, t)| *t);
            match Trajectory::new(pts) {
                Ok(trajectory) => {
                    match self
                        .vehicles
                        .iter_mut()
                        .find(|v| v.original_id == vehicle_name)
                    {
                        Some(vehicle) => {
                            modified += 1;
                            vehicle.alt_trajectory = Some(trajectory);
                        }
                        None => {
                            warn!(
                                "Ticketing data refers to unknown vehicle {:?}",
                                vehicle_name
                            );
                        }
                    }
                }
                Err(err) => {
                    warn!(
                        "Couldn't make trajectory from ticketing for {:?}: {}",
                        vehicle_name, err
                    );
                }
            }
        }
        info!(
            "Overrode trajectories for {modified} / {} vehicles",
            self.vehicles.len()
        );
    }
}
