use std::collections::BTreeSet;

use widgetry::{Autocomplete, EventCtx, GfxCtx, Line, Outcome, Panel, State, Widget};

use model::gtfs::{DateFilter, RouteVariantID, VariantFilter};

use super::filters::Filters;
use super::Viewer;
use crate::{App, Transition};

pub struct SearchForRouteVariant {
    panel: Panel,
}

impl SearchForRouteVariant {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        variants: BTreeSet<RouteVariantID>,
    ) -> Box<dyn State<App>> {
        let mut entries = Vec::new();
        for id in variants {
            let variant = app.model.gtfs.variant(id);
            entries.push((variant.describe(&app.model.gtfs), id));
        }
        Box::new(Self {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Search for a route variant")
                        .small_heading()
                        .into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Autocomplete::new_widget(ctx, entries, 10).named("search"),
            ]))
            .build(ctx),
        })
    }
}

impl State<App> for SearchForRouteVariant {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        if let Some(mut variants) = self.panel.autocomplete_done::<RouteVariantID>("search") {
            if variants.is_empty() {
                return Transition::Pop;
            }
            let variant = variants.remove(0);
            return Transition::Multi(vec![
                Transition::Pop,
                Transition::ModifyState(Box::new(move |state, ctx, app| {
                    app.filters = Filters {
                        filter: VariantFilter {
                            date_filter: DateFilter::None,
                            minimum_trips_per_day: 0,
                        },
                        variant: Some(variant),
                    };

                    state
                        .downcast_mut::<Viewer>()
                        .unwrap()
                        .on_filter_change(ctx, app);
                })),
            ]);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}
