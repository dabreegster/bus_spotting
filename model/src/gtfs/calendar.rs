use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use chrono::NaiveDate;
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
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,

    pub extra_days: BTreeSet<NaiveDate>,
    pub removed_days: BTreeSet<NaiveDate>,
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
                start_date: NaiveDate::parse_from_str(&rec.start_date, "%Y%m%d")?,
                end_date: NaiveDate::parse_from_str(&rec.end_date, "%Y%m%d")?,

                extra_days: BTreeSet::new(),
                removed_days: BTreeSet::new(),
            },
        );
    }
    Ok(calendar)
}

pub fn load_exceptions<R: std::io::Read>(calendar: &mut Calendar, reader: R) -> Result<()> {
    for rec in csv::Reader::from_reader(reader).deserialize() {
        let rec: DateRecord = rec?;
        let service = if let Some(x) = calendar.services.get_mut(&rec.service_id) {
            x
        } else {
            bail!("Exception for unknown {:?}", rec.service_id);
        };
        let date = NaiveDate::parse_from_str(&rec.date, "%Y%m%d")?;
        if rec.exception_type == 1 {
            service.extra_days.insert(date);
        } else if rec.exception_type == 2 {
            service.removed_days.insert(date);
        } else {
            bail!("Unknown exception_type {}", rec.exception_type);
        }
    }
    Ok(())
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

#[derive(Deserialize)]
struct DateRecord {
    service_id: ServiceID,
    date: String,
    exception_type: u8,
}
