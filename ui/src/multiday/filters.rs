use std::collections::BTreeSet;

use widgetry::{
    include_labeled_bytes, Choice, EventCtx, Image, Line, Panel, Spinner, TextBox, TextExt, Widget,
};

use gtfs::{DateFilter, RouteVariantID, VariantFilter};

use super::App;
use crate::components::{date_filter, describe};

pub struct Filters {
    pub filter: VariantFilter,
    pub variant: Option<RouteVariantID>,
}

impl Filters {
    pub fn new() -> Self {
        Self {
            filter: VariantFilter {
                date_filter: DateFilter::None,
                minimum_trips_per_day: 0,
                description_substring: String::new(),
            },
            variant: None,
        }
    }

    pub fn to_controls(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        let mut col = Vec::new();
        col.push(Widget::row(vec![
            Image::from_bytes(include_labeled_bytes!("../../assets/filter.svg")).into_widget(ctx),
            Line(format!(
                "{} total route variants",
                app.model.gtfs.all_variants().len()
            ))
            .secondary()
            .into_widget(ctx),
        ]));
        col.push(date_filter::to_controls(ctx, &self.filter.date_filter).section(ctx));

        col.push(Widget::row(vec![
            "Minimum trips per day".text_widget(ctx),
            Spinner::widget(
                ctx,
                "trips_per_day",
                (0, 100),
                self.filter.minimum_trips_per_day,
                1,
            ),
        ]));

        // List all route variants matching the filters
        let variants = app.model.gtfs.variants_matching_filter(&self.filter);

        let mut variant_choices = vec![Choice::new("all route variants", None)];
        for v in &variants {
            variant_choices.push(Choice::new(
                app.model.gtfs.variant(*v).describe(&app.model.gtfs),
                Some(*v),
            ));
        }
        col.push(Widget::row(vec![
            "Route description:".text_widget(ctx),
            TextBox::widget(
                ctx,
                "description_substring",
                self.filter.description_substring.clone(),
                false,
                10,
            ),
            ctx.style()
                .btn_plain
                .icon_bytes(include_labeled_bytes!("../../assets/reset.svg"))
                .build_widget(ctx, "reset route description filter"),
        ]));

        col.push(Widget::row(vec![
            format!("{} route variants", variants.len()).text_widget(ctx),
            Widget::dropdown(ctx, "variant", self.variant, variant_choices),
        ]));

        if let Some(v) = self.variant {
            let variant = app.model.gtfs.variant(v);
            col.push(describe::route(&app.model.gtfs.routes[&variant.route_id]).into_widget(ctx));
            col.push(
                describe::service(&app.model.gtfs.calendar.services[&variant.service_id])
                    .into_widget(ctx),
            );
        }

        Widget::col(col).section(ctx)
    }

    pub fn from_controls(app: &App, p: &Panel) -> Option<Self> {
        let date_filter = date_filter::from_controls(p)?;
        let minimum_trips_per_day = p.spinner("trips_per_day");
        let mut variant: Option<RouteVariantID> = p.dropdown_value("variant");
        let description_substring = p.text_box("description_substring");

        // If the user changed filters, it may exclude this chosen variant
        if let Some(v) = variant {
            let v = app.model.gtfs.variant(v);
            let service = &app.model.gtfs.calendar.services[&v.service_id];
            if !service.matches_date(&date_filter) || v.trips.len() < minimum_trips_per_day {
                variant = None;
            }
        }

        Some(Self {
            filter: VariantFilter {
                date_filter,
                minimum_trips_per_day,
                description_substring,
            },
            variant,
        })
    }

    // TODO Weird to live here, when app.filters is probably self?
    pub fn selected_variants(&self, app: &App) -> BTreeSet<RouteVariantID> {
        if let Some(v) = self.variant {
            vec![v].into_iter().collect()
        } else {
            app.model.gtfs.variants_matching_filter(&self.filter)
        }
    }
}
