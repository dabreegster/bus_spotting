use std::collections::BTreeMap;

use anyhow::Result;
use geom::Time;

use crate::{Model, VehicleName};

impl Model {
    // TODO Not sure what this should fill out yet.
    pub fn segment(&self) -> Result<()> {
        // We're assuming the model only represents one day right now

        let mut vehicles: BTreeMap<VehicleName, Assignment> = BTreeMap::new();
        for journey in &self.journeys {
            for leg in &journey.legs {
                // Somebody boarded a particular vehicle at some time, and the record claims that
                // vehicle was serving a particular route
                vehicles
                    .entry(leg.vehicle_name.clone())
                    .or_insert_with(Assignment::new)
                    .update(leg.time, &leg.route_short_name);
            }
        }

        for assignment in vehicles.values_mut() {
            assignment.segments.sort_by_key(|(t1, _, _)| *t1);
            // TODO Check for overlaps
        }

        for (vehicle, assignment) in vehicles {
            // Most serve 1 route; that's the simple case for matching
            if assignment.segments.len() == 1 {
                continue;
            }

            println!("{:?} serves {} routes", vehicle, assignment.segments.len());
            for (t1, t2, route) in assignment.segments {
                println!("  - from {t1} to {t2}: {route}");
            }
        }

        // Manually debug vehicles assigned to multiple routes with apparent overlap
        // To understand best, then do `sort -n vehicle_assignment.csv`
        if false && cfg!(not(target_arch = "wasm32")) {
            use std::fs::File;
            use std::io::Write;

            let mut f = File::create("vehicle_assignment.csv")?;
            writeln!(f, "time,route_short_name")?;
            let debug = VehicleName("03279".to_string());

            for journey in &self.journeys {
                for leg in &journey.legs {
                    if leg.vehicle_name == debug {
                        writeln!(f, "{},{}", leg.time.inner_seconds(), leg.route_short_name)?;
                    }
                }
            }
        }

        Ok(())
    }
}

struct Assignment {
    // (time1, time2, route short name)
    // Ordered by time1
    segments: Vec<(Time, Time, String)>,
}

impl Assignment {
    fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    fn update(&mut self, time: Time, route: &str) {
        // Do we already serve this route?
        for (t1, t2, route_short_name) in &mut self.segments {
            if route != route_short_name {
                continue;
            }
            // Extend the time interval if needed
            if time < *t1 {
                *t1 = time;
            } else if time > *t2 {
                *t2 = time;
            }
            return;
        }

        // This vehicle is serving another route. Start an empty interval for it.
        self.segments.push((time, time, route.to_string()));
    }
}
