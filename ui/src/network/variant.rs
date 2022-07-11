use widgetry::{
    DrawBaselayer, EventCtx, GfxCtx, Line, Outcome, Panel, State, Text, TextExt, Widget,
};

use model::gtfs::{RouteVariant, RouteVariantID};

use crate::{App, Transition};

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
                table(ctx, app, variant),
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

fn table(ctx: &mut EventCtx, app: &App, variant: &RouteVariant) -> Widget {
    let mut headers = Vec::new();
    headers.push("Vehicle".text_widget(ctx));
    for idx in 0..variant.stops().len() {
        headers.push(format!("Stop {}", idx + 1).text_widget(ctx));
    }

    let mut rows = Vec::new();
    'TRIP: for trip in &variant.trips {
        let mut row = Vec::new();
        for stop_time in &trip.stop_times {
            let mut txt = Text::from(format!("{}", stop_time.arrival_time));
            if let Some(event) = app.model.find_boarding_event(trip.id, stop_time.stop_id) {
                if row.is_empty() {
                    // Show what vehicle served this trip
                    let (mut entry, hitbox) = Text::from(Line(format!("{:?}", event.vehicle)))
                        .render_autocropped(ctx)
                        .batch()
                        .container()
                        .padding(10.0)
                        .into_geom(ctx, None);
                    entry.push(Color::RED.alpha(0.2), hitbox);
                    row.push(entry);
                }

                txt.add_line(Line(format!("Actually {}", event.arrival_time)));
                txt.add_line(Line(super::compare_time(
                    stop_time.arrival_time,
                    event.arrival_time,
                )));
                if event.new_riders.len() + event.transfers.len() > 0 {
                    txt.add_line(Line(format!(
                        "+{}, {}",
                        event.new_riders.len(),
                        event.transfers.len()
                    )));
                }
            } else {
                // Skip unmatched trips for now, to make the table display less overwhelming
                continue 'TRIP;
            }

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

use geom::Polygon;
use widgetry::{Color, ControlState, GeomBatch};

// TODO This is copied from widgetry to update it; move it back there
fn render_table(
    ctx: &mut EventCtx,
    headers: Vec<Widget>,
    rows: Vec<(String, Vec<GeomBatch>)>,
    total_width: f64,
    min_extra_margin: f64,
) -> Widget {
    let mut width_per_col: Vec<f64> = headers.iter().map(|w| w.get_width_for_forcing()).collect();
    for (_, row) in &rows {
        for (col, width) in row.iter().zip(width_per_col.iter_mut()) {
            *width = width.max(col.get_dims().width);
        }
    }

    let actual_total_width = width_per_col.clone().into_iter().sum::<f64>();
    let extra_margin = ((total_width - actual_total_width) / (width_per_col.len() - 1) as f64)
        .max(min_extra_margin);
    //println!("total_width wanted {total_width}, actual {actual_total_width}, extra margin {extra_margin}");

    let mut col = vec![Widget::custom_row(
        headers
            .into_iter()
            .enumerate()
            .map(|(idx, w)| {
                let margin = extra_margin + width_per_col[idx] - w.get_width_for_forcing();
                //println!("margin for col {idx} is {margin}. {extra_margin} + {} - {}", width_per_col[idx], w.get_width_for_forcing());
                if idx == width_per_col.len() - 1 {
                    w.margin_right((margin - extra_margin) as usize)
                } else {
                    w.margin_right(margin as usize)
                }
            })
            .collect(),
    )];

    // TODO Maybe can do this now simpler with to_geom
    for (label, row) in rows {
        let mut batch = GeomBatch::new();
        batch.autocrop_dims = false;
        let mut x1 = 0.0;
        for (col, width) in row.into_iter().zip(width_per_col.iter()) {
            batch.append(col.translate(x1, 0.0));
            x1 += *width + extra_margin;
        }

        // TODO What if we exceed this?
        let rect = Polygon::rectangle(total_width, batch.get_dims().height);
        let mut hovered = GeomBatch::new();
        hovered.push(Color::hex("#7C7C7C"), rect.clone());
        hovered.append(batch.clone());

        col.push(
            ctx.style()
                .btn_plain
                .btn()
                .custom_batch(batch, ControlState::Default)
                .custom_batch(hovered, ControlState::Hovered)
                .no_tooltip()
                .build_widget(ctx, &label),
        );
    }

    Widget::custom_col(col)
}
