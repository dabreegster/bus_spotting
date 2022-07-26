use std::fs::File;
use std::io::Write;

use anyhow::Result;
use geom::{Distance, Duration, GPSBounds, Line, PolyLine, Pt2D, Speed, Time};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Trajectory {
    // TODO Figure out how to represent/compress staying in the same position for a long time
    inner: Vec<(Pt2D, Time)>,
}

// Creation
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

    /// Makes up nonsense times per point
    // TODO Remove this
    pub fn from_polyline(pl: &PolyLine) -> Self {
        let mut time = Time::START_OF_DAY;
        let mut inner = Vec::new();
        for pt in pl.points() {
            inner.push((*pt, time));
            time += Duration::minutes(1);
        }
        Self { inner }
    }

    pub fn from_pieces_with_times(pieces: &Vec<PolyLine>, times: Vec<Time>) -> Result<Self> {
        if pieces.len() != times.len() - 1 {
            bail!("{} PolyLines, but {} times", pieces.len(), times.len());
        }

        let mut inner = Vec::new();
        for (pair, pl) in times.windows(2).zip(pieces) {
            let trajectory = Self::lerp_along_pl(pl, pair[0], pair[1]);
            inner.extend(trajectory.inner);
        }
        Self::new(inner)
    }

    fn lerp_along_pl(pl: &PolyLine, t1: Time, t2: Time) -> Self {
        let mut inner = Vec::new();
        let total_dist = pl.length();
        let mut dist_so_far = Distance::ZERO;
        let mut last_pt = pl.first_pt();
        for pt in pl.points() {
            let pt = *pt;
            dist_so_far += last_pt.dist_to(pt);
            last_pt = pt;
            let pct = dist_so_far / total_dist;
            let time = t1 + pct * (t2 - t1);
            inner.push((pt, time));
        }
        Self { inner }
    }

    /// Split a trajectory into pieces every time it crosses itself. This will split when a vehicle
    /// drives on a bridge over a previous part of its path.
    pub fn split_non_overlapping(&self) -> Vec<Trajectory> {
        let mut results = Vec::new();

        let mut current_trajectory = Trajectory { inner: Vec::new() };
        let mut current_pl: Option<PolyLine> = None;

        for (pt, t) in &self.inner {
            let pt = *pt;
            let t = *t;

            match current_pl {
                Some(ref current) => {
                    // We might be looking at two equal adjacent points. That's not a split.
                    if let Ok(pl) = PolyLine::new(vec![current.last_pt(), pt]) {
                        let has_intersection = if current.points().len() == 2 {
                            false
                        } else {
                            // Either there's noise in the trajectory data or the intersection
                            // check is too sensitive. Ignore the last point.
                            let mut pts = current.clone().into_points();
                            pts.pop().unwrap();
                            let compare = PolyLine::must_new(pts);
                            compare.intersection(&pl).is_some()
                        };

                        // If adding this point causes the polyline to intersect itself, we've found a split
                        if has_intersection {
                            results.push(current_trajectory.clone());
                            let last = current_trajectory.inner.last().unwrap().clone();
                            current_trajectory.inner = vec![last, (pt, t)];
                            current_pl = Some(pl);
                            continue;
                        }
                    }
                    current_trajectory.inner.push((pt, t));
                    current_pl = Some(current_pl.take().unwrap().optionally_push(pt));
                }
                None => {
                    // Still at the beginning
                    if let Some(last) = current_trajectory.inner.last() {
                        current_pl = PolyLine::new(vec![last.0, pt]).ok();
                    }
                    current_trajectory.inner.push((pt, t));
                }
            }
        }
        results.push(current_trajectory);
        results
    }

    pub fn clip_to_time(&self, t1: Time, t2: Time) -> Result<Self> {
        let mut inner = Vec::new();
        for (pt, t) in &self.inner {
            let pt = *pt;
            let t = *t;
            if t < t1 {
                continue;
            }
            if t > t2 {
                break;
            }
            if inner.is_empty() {
                if let Some((pt, _)) = self.interpolate(t1) {
                    inner.push((pt, t1));
                }
            }
            inner.push((pt, t));
        }
        if let Some((pt, _)) = self.interpolate(t2) {
            inner.push((pt, t2));
        }

        Self::new(inner)
    }
}

// Queries
impl Trajectory {
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
    // the point. If the trajectory stays near the same point for a while, returns the earliest time
    pub fn times_near_pos(&self, pos: Pt2D, threshold: Distance) -> Vec<(Time, Pt2D)> {
        // TODO Maybe FindClosest
        let mut hits = Vec::new();
        for pair in self.inner.windows(2) {
            if let Ok(pl) = PolyLine::new(vec![pair[0].0, pair[1].0]) {
                let pt_on_pl = pl.project_pt(pos);
                if pos.dist_to(pt_on_pl) < threshold {
                    if let Some((dist, _)) = pl.dist_along_of_point(pt_on_pl) {
                        let pct = dist / pl.length();
                        let t1 = pair[0].1;
                        let t2 = pair[1].1;
                        hits.push((t1 + pct * (t2 - t1), pt_on_pl));
                    }
                }
            }
        }

        // Dedupe by time if the trajectory stays near the same point for a while
        let time_threshold = Duration::seconds(30.0);
        let mut results = Vec::new();
        for (t, pt) in hits {
            if results
                .last()
                .map(|(last_t, _)| t - *last_t > time_threshold)
                .unwrap_or(true)
            {
                results.push((t, pt));
            }
        }
        results
    }
}

// Comparing trajectories. Lower results are more similar.
impl Trajectory {
    /// Sum distance from points at these times
    pub fn score_at_points(&self, expected: Vec<(Time, Pt2D)>) -> Option<Distance> {
        let mut sum = Distance::ZERO;
        for (t, pt1) in expected {
            if let Some((pt2, _)) = self.interpolate(t) {
                sum += pt1.dist_to(pt2);
            } else {
                // If the vehicle wasn't even around at this time, probably not a match
                return None;
            }
        }
        Some(sum)
    }

    /// Ignore time. Take the shorter polyline, and walk along it every few meters. Compare to the
    /// equivalent position along the other.
    pub fn score_by_position(&self, other: &Trajectory) -> Distance {
        let step_size = Distance::meters(100.0);
        let buffer_ends = Distance::ZERO;

        let mut sum = Distance::ZERO;
        for ((pt1, _), (pt2, _)) in self
            .as_polyline()
            .step_along(step_size, buffer_ends)
            .into_iter()
            .zip(other.as_polyline().step_along(step_size, buffer_ends))
        {
            sum += pt1.dist_to(pt2);
        }
        sum
    }

    pub fn write_to_csv(&self, path: String, gps_bounds: &GPSBounds) -> Result<()> {
        let mut f = File::create(path)?;
        writeln!(f, "time,longitude,latitude")?;
        for (pt, time) in &self.inner {
            let gps = pt.to_gps(gps_bounds);
            writeln!(f, "{},{},{}", time.inner_seconds(), gps.x(), gps.y())?;
        }
        Ok(())
    }
}
