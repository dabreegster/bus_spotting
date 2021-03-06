use widgetry::{Line, Text};

use gtfs::{Route, Service, Stop};

pub fn stop(stop: &Stop) -> Text {
    let mut txt = Text::from(format!("{:?} ({:?})", stop.orig_id, stop.id));
    if let Some(ref name) = stop.name {
        txt.add_line(Line(format!("Name: {name}")));
    }
    if let Some(ref code) = stop.code {
        txt.add_line(Line(format!("Code: {code}")));
    }
    if let Some(ref description) = stop.description {
        txt.add_line(Line(format!("Description: {description}")));
    }
    txt
}

pub fn route(route: &Route) -> Text {
    let mut txt = Text::from(format!("{:?}", route.route_id));
    if let Some(ref x) = route.short_name {
        txt.add_line(Line(format!("Short name: {x}")));
    }
    if let Some(ref x) = route.long_name {
        txt.add_line(Line(format!("Long name: {x}")));
    }
    if let Some(ref x) = route.description {
        txt.add_line(Line(format!("Description: {x}")));
    }
    txt
}

pub fn service(service: &Service) -> Text {
    let mut txt = Text::from(format!("{:?}", service.service_id));
    txt.add_line(Line(format!(
        "Operates {}",
        service.days_of_week.describe()
    )));
    txt.add_line(Line(format!(
        "{} - {}",
        service.start_date, service.end_date
    )));
    let extra = service.extra_days.len();
    let removed = service.removed_days.len();
    if extra + removed > 0 {
        txt.add_line(Line(format!("({extra} days added, {removed} removed)")));
    }
    txt
}
