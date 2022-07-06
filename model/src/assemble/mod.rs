// Stuff to assemble the AVL, GTFS, and BIL data together to tell a coherent story.

mod boarding;
mod to_trips;
mod vehicle_to_routes;

pub use boarding::{populate_boarding, BoardingEvent};
pub use to_trips::{ActualTrip, BUS_TO_STOP_THRESHOLD};
