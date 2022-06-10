use widgetry::{include_labeled_bytes, lctrl, Choice, EventCtx, Key, Panel, TextExt, Widget};

use model::gtfs::{DateFilter, RouteVariantID};

use crate::components::{date_filter, describe};
use crate::App;

pub struct Filters {
    pub date_filter: DateFilter,
    pub variant: Option<RouteVariantID>,
}

impl Filters {
    pub fn new() -> Self {
        Self {
            date_filter: DateFilter::None,
            variant: None,
        }
    }

    pub fn to_controls(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        let mut col = Vec::new();
        col.push(date_filter::to_controls(ctx, &self.date_filter).section(ctx));

        // List all route variants matching the dates
        let variants = app.model.gtfs.variants_matching_dates(&self.date_filter);

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
        let mut variant: Option<RouteVariantID> = p.dropdown_value("variant");

        // If the user changed the date filter, it may exclude this variant
        if let Some(v) = variant {
            let service = &app.model.gtfs.calendar.services[&app.model.gtfs.variant(v).service_id];
            if !service.matches_date(&date_filter) {
                variant = None;
            }
        }

        Some(Self {
            date_filter,
            variant,
        })
    }
}
