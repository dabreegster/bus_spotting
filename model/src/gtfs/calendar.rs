use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ServiceID(String);

#[derive(Serialize, Deserialize)]
pub struct Calendar {
    pub services: BTreeMap<ServiceID, Service>,
    // TODO All the exceptions
}

#[derive(Serialize, Deserialize)]
pub struct Service {
    pub service_id: ServiceID,
    pub monday: bool,
    pub tuesday: bool,
    pub wednesday: bool,
    pub thursday: bool,
    pub friday: bool,
    pub saturday: bool,
    pub sunday: bool,
    // TODO Real types...
    pub start_date: String,
    pub end_date: String,
}

impl Calendar {
    // TODO get all the valid service IDs for a date. UI can use that to filter stuff.
}

impl Service {
    pub fn describe_days(&self) -> String {
        let weekdays = [
            self.monday,
            self.tuesday,
            self.wednesday,
            self.thursday,
            self.friday,
        ]
        .into_iter()
        .filter(|x| *x)
        .count();
        let weekends = [self.saturday, self.sunday]
            .into_iter()
            .filter(|x| *x)
            .count();
        if weekdays + weekends == 7 {
            return "every day".to_string();
        }
        if weekdays == 5 && weekends == 0 {
            return "weekdays".to_string();
        }
        if weekdays == 0 && weekends == 2 {
            return "weekends".to_string();
        }
        if weekdays == 0 && weekends == 0 {
            // TODO Maybe this is solely defined by exceptions
            return "never?!".to_string();
        }
        let mut result = String::new();
        for (day, operates) in [
            ("M", self.monday),
            ("T", self.tuesday),
            ("W", self.wednesday),
            ("Th", self.thursday),
            ("F", self.friday),
            ("Sat", self.saturday),
            ("Sun", self.sunday),
        ] {
            if operates {
                result.push_str(day);
            }
        }
        result
    }
}

pub fn load<R: std::io::Read>(reader: R) -> Result<Calendar> {
    let mut calendar = Calendar {
        services: BTreeMap::new(),
    };
    for rec in csv::Reader::from_reader(reader).deserialize() {
        let rec: Record = rec?;
        if calendar.services.contains_key(&rec.service_id) {
            bail!("Duplicate {:?}", rec.service_id);
        }
        calendar.services.insert(
            rec.service_id.clone(),
            Service {
                service_id: rec.service_id,
                monday: rec.monday,
                tuesday: rec.tuesday,
                wednesday: rec.wednesday,
                thursday: rec.thursday,
                friday: rec.friday,
                saturday: rec.saturday,
                sunday: rec.sunday,
                start_date: rec.start_date,
                end_date: rec.end_date,
            },
        );
    }
    Ok(calendar)
}

#[derive(Deserialize)]
struct Record {
    service_id: ServiceID,
    #[serde(deserialize_with = "parse_bool")]
    monday: bool,
    #[serde(deserialize_with = "parse_bool")]
    tuesday: bool,
    #[serde(deserialize_with = "parse_bool")]
    wednesday: bool,
    #[serde(deserialize_with = "parse_bool")]
    thursday: bool,
    #[serde(deserialize_with = "parse_bool")]
    friday: bool,
    #[serde(deserialize_with = "parse_bool")]
    saturday: bool,
    #[serde(deserialize_with = "parse_bool")]
    sunday: bool,
    start_date: String,
    end_date: String,
}

fn parse_bool<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let n = <u8>::deserialize(d)?;
    if n == 1 {
        return Ok(true);
    }
    if n == 0 {
        return Ok(false);
    }
    Err(serde::de::Error::custom(format!("Unknown bool value {n}")))
}
