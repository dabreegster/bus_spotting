use anyhow::Result;
use geom::{Distance, Line, PolyLine, Pt2D, Speed, Time};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
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

    // Returns all the (times, snapped points) when the trajectory passes within some threshold of
    // the point.
    // TODO Should dedupe by time
    pub fn times_near_pos(&self, pos: Pt2D, threshold: Distance) -> Vec<(Time, Pt2D)> {
        // TODO Maybe FindClosest
        let mut hits = Vec::new();
        for pair in self.inner.windows(2) {
            if let Ok(pl) = PolyLine::new(vec![pair[0].0, pair[1].0]) {
                let pt_on_pl = pl.project_pt(pos);
                if pos.dist_to(pt_on_pl) < threshold {
                    if let Some((dist, _)) = pl.dist_along_of_point(pl.project_pt(pos)) {
                        let pct = dist / pl.length();
                        let t1 = pair[0].1;
                        let t2 = pair[1].1;
                        hits.push((t1 + pct * (t2 - t1), pt_on_pl));
                    }
                }
            }
        }
        hits
    }
}
