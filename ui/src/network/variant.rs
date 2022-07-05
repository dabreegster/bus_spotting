use widgetry::{
    DrawBaselayer, EventCtx, GfxCtx, Line, Outcome, Panel, State, Text, TextExt, Widget,
};

use model::gtfs::RouteVariant;

use crate::{App, Transition};

pub struct VariantInfo {
    panel: Panel,
}

impl VariantInfo {
    pub fn new_state(ctx: &mut EventCtx, app: &App, variant: &RouteVariant) -> Box<dyn State<App>> {
        Box::new(Self {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line(format!("{:?}", variant.variant_id))
                        .small_heading()
                        .into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                variant.describe(&app.model.gtfs).text_widget(ctx),
                table(ctx, app, variant),
            ]))
            .build(ctx),
        })
    }
}

impl State<App> for VariantInfo {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                // Can't click trips yet
                _ => {}
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        // Probably show just this route?
        DrawBaselayer::PreviousState
    }
}

fn table(ctx: &mut EventCtx, app: &App, variant: &RouteVariant) -> Widget {
    let mut headers = Vec::new();
    for idx in 0..variant.stops().len() {
        headers.push(format!("Stop {}", idx + 1).text_widget(ctx));
    }

    let mut rows = Vec::new();
    for trip in &variant.trips {
        let mut row = Vec::new();
        for stop_time in &trip.stop_times {
            row.push(Text::from(format!("{}", stop_time.arrival_time)).render(ctx));
        }
        rows.push((format!("{:?}", trip.id), row));
    }

    widgetry::table::render_table(ctx, headers, rows, 0.9 * ctx.canvas.window_width)
}
