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
                _ => unreachable!(),
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
        // TODO The tooltip sticks around, and also, this isn't what we want to show
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
        if let Some(actual) = app.model.find_event(&trip.trip_id, stop.id) {
            let actual = actual.arrival_time;
            txt.add_line(Line(format!(
                "{} (actually {} -- {})",
                scheduled,
                actual,
                compare_time(scheduled, actual)
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
