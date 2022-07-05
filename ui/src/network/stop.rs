use abstutil::Counter;
use geom::Time;
use widgetry::{
    Choice, DrawBaselayer, EventCtx, GfxCtx, Line, Outcome, Panel, State, Text, TextExt, Widget,
};

use model::gtfs::{RouteVariant, RouteVariantID, Stop, StopID};

use crate::components::describe;
use crate::{App, Transition};

pub struct StopInfo {
    panel: Panel,
    stop_id: StopID,
    variants: Vec<RouteVariantID>,
}

impl StopInfo {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        stop: &Stop,
        variants: Vec<RouteVariantID>,
        variant: RouteVariantID,
    ) -> Box<dyn State<App>> {
        Box::new(Self {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Stop").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                describe::stop(stop).into_widget(ctx),
                Widget::row(vec![
                    format!("{} variants", variants.len()).text_widget(ctx),
                    Widget::dropdown(
                        ctx,
                        "variant",
                        variant,
                        variants
                            .iter()
                            .map(|v| {
                                Choice::new(
                                    app.model.gtfs.variant(*v).describe(&app.model.gtfs),
                                    *v,
                                )
                            })
                            .collect(),
                    ),
                ]),
                total_counts_per_variant(ctx, app, stop).section(ctx),
                schedule(ctx, app, stop, app.model.gtfs.variant(variant)),
            ]))
            .build(ctx),
            variants,
            stop_id: stop.id,
        })
    }
}

impl State<App> for StopInfo {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                x => {
                    if let Some(x) = x.strip_prefix("Variant ") {
                        let variant = RouteVariantID(x.parse::<usize>().unwrap());
                        return Transition::Replace(Self::new_state(
                            ctx,
                            app,
                            &app.model.gtfs.stops[&self.stop_id],
                            std::mem::take(&mut self.variants),
                            variant,
                        ));
                    } else {
                        unreachable!()
                    }
                }
            },
            Outcome::Changed(_) => {
                return Transition::Replace(Self::new_state(
                    ctx,
                    app,
                    &app.model.gtfs.stops[&self.stop_id],
                    std::mem::take(&mut self.variants),
                    self.panel.dropdown_value("variant"),
                ));
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        // Probably show all routes connected to this stop?
        DrawBaselayer::PreviousState
    }
}

fn schedule(ctx: &mut EventCtx, app: &App, stop: &Stop, variant: &RouteVariant) -> Widget {
    let mut txt = Text::new();
    txt.add_line(Line("Schedule").small_heading());
    txt.add_line(Line(""));
    for trip in &variant.trips {
        let scheduled = trip.arrival_at(stop.id);
        if let Some(actual) = app.model.find_boarding_event(trip.id, stop.id) {
            txt.add_line(Line(format!(
                "{} (actually {} -- {}) -- {} new riders, {} transfers by {:?}",
                scheduled,
                actual.arrival_time,
                compare_time(scheduled, actual.arrival_time),
                actual.new_riders.len(),
                actual.transfers.len(),
                actual.vehicle,
            )));
        } else {
            txt.add_line(Line(format!("{} (no real data)", scheduled)));
        }
    }
    txt.into_widget(ctx)
}

fn compare_time(scheduled: Time, actual: Time) -> String {
    if scheduled == actual {
        return "on time".to_string();
    }
    if scheduled < actual {
        return format!("{} late", actual - scheduled);
    }
    format!("{} early", scheduled - actual)
}

// TODO Use the variants list to filter by day
fn total_counts_per_variant(ctx: &mut EventCtx, app: &App, stop: &Stop) -> Widget {
    let mut trips_per_variant = Counter::new();
    let mut new_riders_per_variant = Counter::new();
    let mut transfers_per_variant = Counter::new();

    for event in app.model.all_boarding_events_at_stop(stop.id) {
        trips_per_variant.inc(event.variant);
        new_riders_per_variant.add(event.variant, event.new_riders.len());
        transfers_per_variant.add(event.variant, event.transfers.len());
    }

    let mut col = vec![Line("Totals per variant").small_heading().into_widget(ctx)];
    for (variant, num_trips) in trips_per_variant.consume() {
        col.push(
            ctx.style()
                .btn_plain
                .text(format!(
                    "{:?}: {} trips, {} new riders, {} transfers",
                    variant,
                    num_trips,
                    new_riders_per_variant.get(variant),
                    transfers_per_variant.get(variant)
                ))
                .build_widget(ctx, format!("Variant {}", variant.0)),
        );
    }
    Widget::col(col)
}
