use anyhow::Result;
use geom::{PolyLine, Pt2D, Speed, Time};

pub struct Trajectory {
    // Time increases (no equal adjacent pairs)
    inner: Vec<(Pt2D, Time)>,
    // TODO Record the speed and rough direction or not?
}

impl Trajectory {
    pub fn new(raw: Vec<(Pt2D, Time)>) -> Result<Self> {
        // Might be sitting in place for a while, can skip some things
        // Assert times increasing
        todo!()
    }

    pub fn interpolate(&self, time: Time) -> Option<(Pt2D, Speed)> {
        // None if the time is outside range
        todo!()
    }

    pub fn start_time(&self) -> Time {
        todo!()
    }

    pub fn end_time(&self) -> Time {
        todo!()
    }

    pub fn as_polyline(&self) -> PolyLine {
        todo!()
    }

    // To animate, just interpolate over time
}
