use geom::{Duration, Pt2D, Time};

use model::{Model, VehicleName};

pub struct Events {
    events: Vec<Event>,
}

pub struct Event {
    pub time: Time,
    pub pos: Pt2D,
    pub vehicle_name: VehicleName,
    pub route_short_name: String,
    pub first_boarding: bool,
}

impl Event {
    pub fn describe(&self) -> String {
        let boarding = if self.first_boarding {
            "first boarding"
        } else {
            "transfer"
        };
        format!(
            "{} on {} using {:?}",
            boarding, self.route_short_name, self.vehicle_name
        )
    }
}

impl Events {
    pub fn ticketing(model: &Model) -> Self {
        let mut events = Vec::new();
        for journey in &model.journeys {
            for (idx, leg) in journey.legs.iter().enumerate() {
                events.push(Event {
                    time: leg.time,
                    pos: leg.pos,
                    vehicle_name: leg.vehicle_name.clone(),
                    route_short_name: leg.route_short_name.clone(),
                    first_boarding: idx == 0,
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
