use geom::{Duration, Pt2D, Time};

use model::{Model, VehicleName};

pub struct Events {
    events: Vec<Event>,
}

pub struct Event {
    pub time: Time,
    pub pos: Pt2D,
    pub vehicle_name: VehicleName,
    pub description: String,
}

impl Events {
    pub fn ticketing(model: &Model) -> Self {
        let mut events = Vec::new();
        for journey in &model.journeys {
            for (idx, leg) in journey.legs.iter().enumerate() {
                let boarding = if idx == 0 {
                    "first boarding"
                } else {
                    "transfer"
                };

                events.push(Event {
                    time: leg.time,
                    pos: leg.pos,
                    vehicle_name: leg.vehicle_name.clone(),
                    description: format!(
                        "{boarding} on {} using {:?}",
                        leg.route_short_name, leg.vehicle_name
                    ),
                });
            }
        }
        events.sort_by_key(|ev| ev.time);
        Self { events }
    }

    pub fn events_at(&self, time2: Time, lookback: Duration) -> Vec<&Event> {
        let time1 = time2.clamped_sub(lookback);

        // TODO Binary search, or use another data structure, or make this be a stateful cursor

        let mut result = Vec::new();
        for ev in &self.events {
            if ev.time < time1 {
                continue;
            }
            if ev.time > time2 {
                break;
            }
            result.push(ev);
        }
        result
    }
}
