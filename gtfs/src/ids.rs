use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod orig {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct StopID(String);

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
    pub struct TripID(String);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StopID(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TripID(usize);

impl CheapID for StopID {
    fn new(x: usize) -> Self {
        Self(x)
    }
}
impl CheapID for TripID {
    fn new(x: usize) -> Self {
        Self(x)
    }
}

pub trait CheapID: Copy {
    fn new(x: usize) -> Self;
}

#[derive(Serialize, Deserialize)]
pub struct IDMapping<K: Ord, V> {
    orig_to_cheap: BTreeMap<K, V>,
    // We don't need to store the inverse. It's more convenient for each object to own that.
}

impl<K: Clone + std::fmt::Debug + Ord, V: CheapID> IDMapping<K, V> {
    pub fn new() -> Self {
        Self {
            orig_to_cheap: BTreeMap::new(),
        }
    }

    pub fn insert_new(&mut self, orig: K) -> Result<V> {
        let cheap = V::new(self.orig_to_cheap.len());
        if self.orig_to_cheap.insert(orig.clone(), cheap).is_some() {
            bail!("IDMapping::insert_new has duplicate input for {:?}", orig);
        }
        Ok(cheap)
    }

    pub fn insert_idempotent(&mut self, orig: &K) -> V {
        match self.orig_to_cheap.get(orig) {
            Some(x) => *x,
            None => {
                let v = V::new(self.orig_to_cheap.len());
                self.orig_to_cheap.insert(orig.clone(), v);
                v
            }
        }
    }

    pub fn lookup(&self, orig: &K) -> Result<V> {
        match self.orig_to_cheap.get(orig) {
            Some(x) => Ok(*x),
            None => bail!("IDMapping lookup of {:?} failed", orig),
        }
    }

    pub fn borrow(&self) -> &BTreeMap<K, V> {
        &self.orig_to_cheap
    }
}
