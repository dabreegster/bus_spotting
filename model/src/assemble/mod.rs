// Stuff to assemble the AVL, GTFS, and BIL data together to tell a coherent story.

mod boarding;
mod vehicle_to_routes;

pub use boarding::{populate_boarding, BoardingEvent};
