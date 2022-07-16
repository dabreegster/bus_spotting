use abstutil::{prettyprint_usize, Counter};
use widgetry::{Color, EventCtx, Line, Panel, SimpleState, State, Text, TextExt, Widget};

use gtfs::RouteVariantID;

use super::{App, Transition};
use crate::components::render_table;

pub struct Analysis;

impl Analysis {
    pub fn boardings_by_variant(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        // SELECT SUM(new_riders + transfers) over days
        // GROUP BY variant, round_down_hour(arrival_time)

        // Sum the total number of boardings over all days. Group by (variant, binned hour).
        let mut sum = Counter::new();
        for (_, events) in &app.model.boardings_per_day {
            for ev in events {
                sum.add(
                    (ev.variant, ev.arrival_time.get_hours()),
                    ev.new_riders.len() + ev.transfers.len(),
                );
            }
        }

        let mut headers = Vec::new();
        headers.push("Variant".to_string());
        for hour in 0..24 {
            headers.push(format!("Hour {}", hour));
        }

        let mut rows = Vec::new();
        for variant in app.model.gtfs.all_variants() {
            let mut row = vec![Text::from(
                app.model.gtfs.variant(variant).describe(&app.model.gtfs),
            )];
            for hour in 0..24 {
                row.push(Text::from(format!(
                    "{}",
                    prettyprint_usize(sum.get((variant, hour)))
                )));
            }
            rows.push((variant.0.to_string(), row));
        }

        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line(format!(
                    "Total boardings over {} days",
                    app.model.boardings_per_day.len()
                ))
                .small_heading()
                .into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            txt_table(ctx, headers, rows),
        ]))
        .build(ctx);

        <dyn SimpleState<_>>::new_state(panel, Box::new(Analysis))
    }
}

impl SimpleState<App> for Analysis {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        x: &str,
        _: &mut Panel,
    ) -> Transition {
        if x == "close" {
            return Transition::Pop;
        }
        if let Ok(x) = x.parse::<usize>() {
            return Transition::Push(super::variant::VariantInfo::new_state(
                ctx,
                app,
                app.model.gtfs.variant(RouteVariantID(x)),
            ));
        }
        unreachable!()
    }
}

fn txt_table(ctx: &mut EventCtx, headers: Vec<String>, rows: Vec<(String, Vec<Text>)>) -> Widget {
    let mut rendered_rows = Vec::new();
    for (label, row) in rows {
        rendered_rows.push((
            label,
            row.into_iter()
                .map(|txt| {
                    let (mut entry, hitbox) = txt
                        .render_autocropped(ctx)
                        .batch()
                        .container()
                        .padding(10.0)
                        .into_geom(ctx, None);
                    entry.push(Color::RED.alpha(0.2), hitbox);
                    entry
                })
                .collect(),
        ));
    }

    let min_extra_margin = 10.0;
    render_table(
        ctx,
        headers.into_iter().map(|x| x.text_widget(ctx)).collect(),
        rendered_rows,
        0.6 * ctx.canvas.window_width,
        min_extra_margin,
    )
}
