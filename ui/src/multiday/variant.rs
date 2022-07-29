use widgetry::{
    Color, DrawBaselayer, EventCtx, GfxCtx, Line, Outcome, Panel, State, Text, TextExt, Widget,
};

use gtfs::{RouteVariant, RouteVariantID};

use super::{App, Transition};
use crate::components::render_table;

pub struct VariantInfo {
    id: RouteVariantID,
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
                ctx.style()
                    .btn_outline
                    .text("export to GeoJSON")
                    .build_def(ctx),
                table(ctx, variant),
            ]))
            .build(ctx),
            id: variant.variant_id,
        })
    }
}

impl State<App> for VariantInfo {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "export to GeoJSON" => {
                    app.model
                        .gtfs
                        .variant(self.id)
                        .export_to_geojson(
                            format!("route_{}.geojson", self.id.0),
                            &app.model.gtfs,
                            &app.model.gps_bounds,
                        )
                        .unwrap();
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

fn table(ctx: &mut EventCtx, variant: &RouteVariant) -> Widget {
    let mut headers = Vec::new();
    headers.push("Vehicle".text_widget(ctx));
    for idx in 0..variant.stops().len() {
        headers.push(format!("Stop {}", idx + 1).text_widget(ctx));
    }

    let mut rows = Vec::new();
    for trip in &variant.trips {
        let mut row = Vec::new();
        for stop_time in &trip.stop_times {
            let txt = Text::from(format!("{}", stop_time.arrival_time));
            let (mut entry, hitbox) = txt
                .render_autocropped(ctx)
                .batch()
                .container()
                .padding(10.0)
                .into_geom(ctx, None);
            entry.push(Color::RED.alpha(0.2), hitbox);
            row.push(entry);
        }
        rows.push((format!("{:?}", trip.id), row));
    }

    let min_extra_margin = 10.0;
    render_table(
        ctx,
        headers,
        rows,
        0.6 * ctx.canvas.window_width,
        min_extra_margin,
    )
}
