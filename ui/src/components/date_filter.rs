use chrono::{Datelike, NaiveDate};
use widgetry::{Choice, EventCtx, Panel, Spinner, TextExt, Toggle, Widget};

use gtfs::{DateFilter, DaysOfWeek};

pub fn to_controls(ctx: &mut EventCtx, filters: &DateFilter) -> Widget {
    let mut col = vec![Widget::row(vec![
        "Filter dates:".text_widget(ctx),
        Widget::dropdown(
            ctx,
            "filter type",
            match filters {
                DateFilter::None => "none",
                DateFilter::SingleDay(_) => "single day",
                DateFilter::Daily(_) => "daily",
            }
            .to_string(),
            Choice::strings(vec!["none", "single day", "daily"]),
        ),
    ])];
    match filters {
        DateFilter::None => {}
        DateFilter::SingleDay(date) => {
            col.push(Widget::row(vec![
                "Year:".text_widget(ctx),
                Spinner::widget(ctx, "year", (2000, 2030), date.year(), 1),
            ]));
            col.push(Widget::row(vec![
                "Month:".text_widget(ctx),
                Spinner::widget(ctx, "month", (1, 12), date.month(), 1),
            ]));
            col.push(Widget::row(vec![
                "Day:".text_widget(ctx),
                Spinner::widget(ctx, "day", (1, 31), date.day(), 1),
            ]));
        }
        DateFilter::Daily(days) => {
            for (day, enabled) in [
                ("Monday", days.monday),
                ("Tuesday", days.tuesday),
                ("Wednesday", days.wednesday),
                ("Thursday", days.thursday),
                ("Friday", days.friday),
                ("Saturday", days.saturday),
                ("Sunday", days.sunday),
            ] {
                col.push(Toggle::checkbox(ctx, day, None, enabled));
            }
        }
    }
    Widget::col(col)
}

// Fails if the user picks an impossible date
pub fn from_controls(p: &Panel) -> Option<DateFilter> {
    Some(
        match p.dropdown_value::<String, _>("filter type").as_ref() {
            "none" => DateFilter::None,
            "single day" => {
                if !p.has_widget("year") {
                    // We just switched to this, use a default
                    return Some(DateFilter::SingleDay(NaiveDate::from_ymd(2000, 1, 1)));
                }

                let y = p.spinner("year");
                let m = p.spinner("month");
                let d = p.spinner("day");
                DateFilter::SingleDay(NaiveDate::from_ymd_opt(y, m, d)?)
            }
            "daily" => {
                if p.has_widget("Monday") {
                    DateFilter::Daily(DaysOfWeek {
                        monday: p.is_checked("Monday"),
                        tuesday: p.is_checked("Tuesday"),
                        wednesday: p.is_checked("Wednesday"),
                        thursday: p.is_checked("Thursday"),
                        friday: p.is_checked("Friday"),
                        saturday: p.is_checked("Saturday"),
                        sunday: p.is_checked("Sunday"),
                    })
                } else {
                    // Just switched to this, start with all
                    DateFilter::Daily(DaysOfWeek::all())
                }
            }
            _ => unreachable!(),
        },
    )
}
