use geom::Time;

// TODO Unit test

/// A sequence of something, with non-overlapping and sorted time intervals.
///
/// Intervals are "open", aka, it's fine for one interval to end right at 7am and the next to the
/// start right at 7am.
pub struct Timetable<T>(pub Vec<(Time, Time, T)>);

impl<T> Timetable<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn is_free(&self, check: (Time, Time)) -> bool {
        for (t1, t2, _) in &self.0 {
            if overlaps(check, (*t1, *t2)) {
                return false;
            }
        }
        true
    }

    // Assumes is_free is true. Maybe combine them?
    pub fn assign(&mut self, pair: (Time, Time), obj: T) {
        if let Some(idx) = self.0.iter().position(|(t1, _, _)| pair.1 <= *t1) {
            self.0.insert(idx, (pair.0, pair.1, obj));
        } else {
            self.0.push((pair.0, pair.1, obj));
        }
    }
}

fn overlaps(pair1: (Time, Time), pair2: (Time, Time)) -> bool {
    fn contains(t: Time, pair: (Time, Time)) -> bool {
        t > pair.0 && t < pair.1
    }

    contains(pair1.0, pair2)
        || contains(pair1.1, pair2)
        || contains(pair2.0, pair1)
        || contains(pair2.1, pair1)
}
