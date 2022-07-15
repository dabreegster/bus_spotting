use geom::Distance;

use crate::{DailyModel, VehicleID};
use gtfs::TripID;

impl DailyModel {
    pub fn look_for_best_matches_by_pos_and_time(&self) {
        let mut best = Vec::new();
        for vehicle in &self.vehicles {
            let mut scores = self.score_vehicle_similarity_to_trips(vehicle.id);
            if !scores.is_empty() {
                let (trip, score) = scores.remove(0);
                best.push((vehicle.id, trip, score));
            }
        }
        best.sort_by_key(|pair| pair.2);
        println!("Any good matches?");
        for (vehicle, trip, score) in best {
            println!("- {:?} matches to {:?} with score {}", vehicle, trip, score);
        }
    }

    // Just see how closely the AVL matches stop position according to the GTFS timetable.
    //
    // This'll not be a good match when there are delays.
    pub fn score_vehicle_similarity_to_trips(&self, id: VehicleID) -> Vec<(TripID, Distance)> {
        let vehicle_trajectory = &self.vehicles[id.0].trajectory;
        let mut scores = Vec::new();
        for variant in self.vehicle_to_possible_routes(id) {
            for trip in &self.gtfs.variant(variant).trips {
                let mut expected = Vec::new();
                for stop_time in &trip.stop_times {
                    expected.push((
                        stop_time.arrival_time,
                        self.gtfs.stops[&stop_time.stop_id].pos,
                    ));
                }
                if let Some(score) = vehicle_trajectory.score_at_points(expected) {
                    scores.push((trip.id, score));
                }
            }
        }
        scores.sort_by_key(|pair| pair.1);
        scores
    }
}
