use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use geom::Time;

use crate::{DailyModel, Timetable, VehicleID};
use gtfs::{DateFilter, TripID, VariantFilter};

impl DailyModel {
    // Per route short name, we can find all vehicles serving it at least part of the day (supply)
    // and all the GTFS trips supposed to happen (demand). We can then try to match things up,
    // assuming a vehicle can only serve one trip at a time.
    //
    // This could still possibly work, but in many cases, there's nowhere near enough supply.
    pub fn supply_demand_matching(&self) -> Result<()> {
        // The supply: per route short name, find all vehicles that served it at least part of the day
        let mut vehicles_per_route: BTreeMap<String, BTreeSet<VehicleID>> = BTreeMap::new();
        for (vehicle, assignment) in self.vehicle_to_route_short_name()? {
            for (_, _, route) in assignment.segments {
                vehicles_per_route
                    .entry(route)
                    .or_insert_with(BTreeSet::new)
                    .insert(vehicle);
            }
        }

        let mut timetable_per_vehicle: BTreeMap<VehicleID, Timetable<TripID>> = BTreeMap::new();
        for vehicle in &self.vehicles {
            timetable_per_vehicle.insert(vehicle.id, Timetable::new());
        }

        let trips_to_assign = self.get_gtfs_trip_demand();

        // Then per route short name, let's match things up...
        let mut assigned_trips = 0;
        let mut unassigned_trips = 0;
        for (route_short_name, trips) in trips_to_assign {
            let supply = vehicles_per_route
                .remove(&route_short_name)
                .unwrap_or_else(BTreeSet::new);
            assign(
                route_short_name,
                trips,
                supply,
                &mut timetable_per_vehicle,
                &mut assigned_trips,
                &mut unassigned_trips,
            );
        }
        info!("{assigned_trips} assigned, {unassigned_trips} left unassigned");

        Ok(())
    }

    // All trips today, broken down by route short name
    pub fn get_gtfs_trip_demand(&self) -> BTreeMap<String, Vec<(TripID, Time, Time)>> {
        let filter = VariantFilter {
            date_filter: DateFilter::SingleDay(self.date),
            minimum_trips_per_day: 0,
            route_type: None,
            description_substring: String::new(),
        };
        let mut trips_to_assign: BTreeMap<String, Vec<(TripID, Time, Time)>> = BTreeMap::new();
        for variant in &self.gtfs.variants_matching_filter(&filter) {
            let variant = self.gtfs.variant(*variant);
            if let Some(ref route_short_name) = self.gtfs.routes[&variant.route_id].short_name {
                for trip in &variant.trips {
                    let (t1, t2) = trip.time_range();
                    trips_to_assign
                        .entry(route_short_name.clone())
                        .or_insert_with(Vec::new)
                        .push((trip.id, t1, t2));
                }
            }
        }
        trips_to_assign
    }
}

fn assign(
    route_short_name: String,
    demand: Vec<(TripID, Time, Time)>,
    supply: BTreeSet<VehicleID>,
    timetable_per_vehicle: &mut BTreeMap<VehicleID, Timetable<TripID>>,
    assigned_trips: &mut usize,
    unassigned_trips: &mut usize,
) {
    info!(
        "for {}, {} trips to assign to {} vehicles. greedy assignment says {} vehicles needed",
        route_short_name,
        demand.len(),
        supply.len(),
        minimum_vehicles_needed(demand.clone()),
    );

    for (trip, t1, t2) in demand {
        let mut ok = false;
        for vehicle in &supply {
            let tt = timetable_per_vehicle.get_mut(vehicle).unwrap();
            if tt.is_free((t1, t2)) {
                tt.assign((t1, t2), trip);
                *assigned_trips += 1;
                ok = true;
                break;
            }
        }
        if !ok {
            *unassigned_trips += 1;
        }
    }
}

fn minimum_vehicles_needed(demand: Vec<(TripID, Time, Time)>) -> usize {
    // Just try to assign all trips to one vehicle. When we can't, create a second and repeat.
    // TODO I'm not sure the greedy algorithm is optimal here
    let mut timetables: Vec<Timetable<TripID>> = Vec::new();
    for (trip, t1, t2) in demand {
        if let Some(idx) = timetables.iter().position(|tt| tt.is_free((t1, t2))) {
            timetables[idx].assign((t1, t2), trip);
        } else {
            let mut tt = Timetable::new();
            tt.assign((t1, t2), trip);
            timetables.push(tt);
        }
    }
    timetables.len()
}
