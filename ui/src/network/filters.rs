use std::collections::BTreeSet;

use widgetry::{
    include_labeled_bytes, lctrl, Choice, EventCtx, Key, Line, Panel, Spinner, TextExt, Widget,
};

use gtfs::{DateFilter, RouteVariantID, VariantFilter};

use crate::components::{date_filter, describe};
use crate::App;

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
            },
            variant: None,
        }
    }

    pub fn to_controls(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        let mut col = Vec::new();
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
            format!("{} route variants", variants.len()).text_widget(ctx),
            Widget::dropdown(ctx, "variant", self.variant, variant_choices),
            ctx.style()
                .btn_plain
                .icon_bytes(include_labeled_bytes!("../../assets/search.svg"))
                .hotkey(lctrl(Key::F))
                .build_widget(ctx, "search for a route variant"),
        ]));
        col.push(
            Line(format!(
                "{} total route variants",
                app.model.gtfs.all_variants().len()
            ))
            .secondary()
            .into_widget(ctx),
        );

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
