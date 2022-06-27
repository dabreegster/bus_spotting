use anyhow::Result;
use geom::{Distance, Duration, Line, PolyLine, Pt2D, Speed, Time};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
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
                        // If adding this point causes the polyline to intersect itself, we've found a split
                        if current.intersection(&pl).is_some() {
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
}
