use anyhow::Result;
use geom::{Distance, Line, PolyLine, Pt2D, Speed, Time};

pub struct Trajectory {
    // TODO Figure out how to represent/compress staying in the same position for a long time
    inner: Vec<(Pt2D, Time)>,
}

impl Trajectory {
    pub fn new(raw: Vec<(Pt2D, Time)>) -> Result<Self> {
        // Just one validation for now
        for pair in raw.windows(2) {
            // TODO Handle equal time, same or different points
            if pair[0].1 > pair[1].1 {
                bail!(
                    "Trajectory input out-of-order: {} then {}",
                    pair[0].1,
                    pair[1].1
                );
            }
        }
        if raw.len() < 2 {
            bail!("Trajectory doesn't have at least 2 points");
        }
        Ok(Self { inner: raw })
    }

    /// None if the trajectory isn't active at this time
    pub fn interpolate(&self, time: Time) -> Option<(Pt2D, Speed)> {
        if time < self.start_time() || time > self.end_time() {
            return None;
        }

        // TODO Binary search at the very least!
        for pair in self.inner.windows(2) {
            let (pos1, t1) = pair[0];
            let (pos2, t2) = pair[1];
            if time >= t1 && time <= t2 {
                match Line::new(pos1, pos2) {
                    Ok(line) => {
                        let percent = (time - t1) / (t2 - t1);
                        let pos = line.percent_along(percent).unwrap();
                        let speed = Speed::from_dist_time(line.length(), t2 - t1);
                        return Some((pos, speed));
                    }
                    Err(_) => {
                        return Some((pos1, Speed::ZERO));
                    }
                }
            }
        }

        unreachable!()
    }

    pub fn start_time(&self) -> Time {
        self.inner[0].1
    }

    pub fn end_time(&self) -> Time {
        self.inner.last().unwrap().1
    }

    pub fn as_polyline(&self) -> PolyLine {
        let mut pts = Vec::new();
        for (pos, _) in &self.inner {
            pts.push(*pos);
        }
        let pts = Pt2D::approx_dedupe(pts, Distance::meters(1.0));

        // TODO The trajectory usually doubles back on itself. Should we split into multiple
        // segments instead of doing this?
        PolyLine::unchecked_new(pts)
    }
}
