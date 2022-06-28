use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use geom::Time;

use crate::gtfs::{DateFilter, TripID, VariantFilter};
use crate::{Model, VehicleID};

impl Model {
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

        let mut timetable_per_vehicle: BTreeMap<VehicleID, Timetable> = BTreeMap::new();
        for vehicle in &self.vehicles {
            timetable_per_vehicle.insert(vehicle.id, Timetable::new());
        }

        // The demand: all trips today, broken down by route short name
        let filter = VariantFilter {
            date_filter: DateFilter::SingleDay(self.main_date),
            minimum_trips_per_day: 0,
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
}

fn assign(
    route_short_name: String,
    demand: Vec<(TripID, Time, Time)>,
    supply: BTreeSet<VehicleID>,
    timetable_per_vehicle: &mut BTreeMap<VehicleID, Timetable>,
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
    let mut timetables: Vec<Timetable> = Vec::new();
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

// TODO Unit test
struct Timetable(Vec<(Time, Time, TripID)>);

impl Timetable {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn is_free(&self, check: (Time, Time)) -> bool {
        for (t1, t2, _) in &self.0 {
            if overlaps(check, (*t1, *t2)) {
                return false;
            }
        }
        true
    }

    // Assumes is_free is true. Maybe combine them?
    fn assign(&mut self, pair: (Time, Time), trip: TripID) {
        if let Some(idx) = self.0.iter().position(|(t1, _, _)| pair.1 < *t1) {
            self.0.insert(idx, (pair.0, pair.1, trip));
        } else {
            self.0.push((pair.0, pair.1, trip));
        }
    }
}

fn overlaps(pair1: (Time, Time), pair2: (Time, Time)) -> bool {
    fn contains(t: Time, pair: (Time, Time)) -> bool {
        t >= pair.0 && t <= pair.1
    }

    contains(pair1.0, pair2)
        || contains(pair1.1, pair2)
        || contains(pair2.0, pair1)
        || contains(pair2.1, pair1)
}
