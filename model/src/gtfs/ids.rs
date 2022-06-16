use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod orig {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct StopID(String);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StopID(usize);

// TODO Make generic, need traits to construct the cheap type
#[derive(Serialize, Deserialize)]
pub struct IDMapping {
    orig_to_cheap: BTreeMap<orig::StopID, StopID>,
    // We don't need to store the inverse. It's more convenient for each object to own that.
}

impl IDMapping {
    pub fn new() -> Self {
        Self {
            orig_to_cheap: BTreeMap::new(),
        }
    }

    pub fn insert_new(&mut self, orig: orig::StopID) -> Result<StopID> {
        let cheap = StopID(self.orig_to_cheap.len());
        if self.orig_to_cheap.insert(orig.clone(), cheap).is_some() {
            bail!("IDMapping::insert_new has duplicate input for {:?}", orig);
        }
        Ok(cheap)
    }

    pub fn lookup(&self, orig: &orig::StopID) -> Result<StopID> {
        match self.orig_to_cheap.get(orig) {
            Some(x) => Ok(*x),
            None => bail!("IDMapping lookup of {:?} failed", orig),
        }
    }
}
