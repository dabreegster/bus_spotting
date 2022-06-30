use geom::{Circle, Distance, Pt2D, Time};
use model::gtfs::{RouteVariant, RouteVariantID};
use model::Trajectory;
use widgetry::{
    Cached, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Outcome,
    Panel, State, Text, VerticalAlignment, Widget,
};

use crate::{App, Transition};

pub struct Viewer {
    panel: Panel,
    trajectory: Trajectory,
    draw: Drawable,
    snap_to_trajectory: Cached<Pt2D, (Text, Drawable)>,
}

impl Viewer {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        trajectory: Trajectory,
        variant: RouteVariantID,
    ) -> Box<dyn State<App>> {
        let mut draw = GeomBatch::new();

        print_timetable(app, &trajectory, app.model.gtfs.variant(variant));

        // AVL
        draw.push(
            Color::CYAN,
            trajectory
                .as_polyline()
                .make_polygons(Distance::meters(5.0)),
        );

        // The route
        let variant = app.model.gtfs.variant(variant);
        if let Ok(pl) = variant.polyline(&app.model.gtfs) {
            draw.push(
                Color::RED.alpha(0.8),
                pl.make_polygons(Distance::meters(3.0)),
            );
        }

        // Labeled stops
        for (idx, id) in variant.stops().into_iter().enumerate() {
            let pt = app.model.gtfs.stops[&id].pos;
            draw.push(
                Color::BLUE,
                Circle::new(pt, Distance::meters(50.0)).to_polygon(),
            );
            draw.append(
                Text::from(Line(format!("{}", idx + 1)).fg(Color::WHITE))
                    .render(ctx)
                    .centered_on(pt),
            );
        }

        Box::new(Self {
            panel: Panel::new_builder(Widget::col(vec![Widget::row(vec![
                Line("Vehicle + route").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ])]))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx),
            trajectory,
            draw: ctx.upload(draw),
            snap_to_trajectory: Cached::new(),
        })
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            }
        }

        // TODO Refactor parts of this
        self.snap_to_trajectory
            .update(ctx.canvas.get_cursor_in_map_space(), |pt| {
                let mut txt = Text::new();
                let mut batch = GeomBatch::new();
                let hits = self.trajectory.times_near_pos(pt, Distance::meters(30.0));
                if !hits.is_empty() {
                    batch.push(
                        Color::YELLOW,
                        Circle::new(hits[0].1, Distance::meters(30.0)).to_polygon(),
                    );
                    let n = hits.len();
                    for (idx, (time, _)) in hits.into_iter().enumerate() {
                        txt.add_line(Line(format!("Here at {time}")));
                        if idx == 4 {
                            txt.append(Line(format!(" (and {} more times)", n - 5)));
                            break;
                        }
                    }
                }
                (txt, ctx.upload(batch))
            });

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        g.redraw(&self.draw);
        if let Some((txt, draw)) = self.snap_to_trajectory.value() {
            g.redraw(draw);
            g.draw_mouse_tooltip(txt.clone());
        }
    }
}

fn print_timetable(app: &App, trajectory: &Trajectory, variant: &RouteVariant) {
    let mut times_near_stops: Vec<Vec<Time>> = Vec::new();
    let mut min_times = usize::MAX;
    for stop in variant.stops() {
        let threshold = Distance::meters(10.0);
        let stop_pos = app.model.gtfs.stops[&stop].pos;
        let times: Vec<Time> = trajectory
            .times_near_pos(stop_pos, threshold)
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        min_times = min_times.min(times.len());
        times_near_stops.push(times);
    }

    // Assemble into trips
    let mut trips: Vec<Vec<Time>> = Vec::new();

    if false {
        // The naive approach
        for trip_idx in 0..min_times {
            let times: Vec<Time> = times_near_stops
                .iter()
                .map(|times| times[trip_idx])
                .collect();
            trips.push(times);
        }
    } else {
        // Assume the first time at the first stop is correct, then build up from there and always
        // require the time to increase. Skip some times if needed
        let mut skipped = 0;
        let mut last_time = Time::START_OF_DAY;
        'OUTER: loop {
            let mut trip_times = Vec::new();
            for times in &mut times_near_stops {
                // Shift while the first time is too early
                while !times.is_empty() && times[0] < last_time {
                    times.remove(0);
                    skipped += 1;
                }
                if times.is_empty() {
                    break 'OUTER;
                }
                last_time = times.remove(0);
                trip_times.push(last_time);
            }
            trips.push(trip_times);
        }

        println!(
            "For below, skipped {} times at different stops because they're out-of-order",
            skipped
        );
    }

    println!(
        "{} trips along {} stops",
        trips.len(),
        variant.stops().len()
    );
    for times in trips {
        // More compressed, but harder to read
        if false {
            println!(
                "- Trip: {}",
                times
                    .iter()
                    .enumerate()
                    .map(|(idx, t)| format!("{} @ {}", idx + 1, t))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            // Look for impossible bits, show the stops
            for (idx, pair) in times.windows(2).enumerate() {
                if pair[1] < pair[0] {
                    println!(
                        "  - Something funny near stop {} ({}) -> {} ({})",
                        idx + 1,
                        pair[0],
                        idx + 2,
                        pair[1]
                    );
                }
            }
        }

        println!(
            "--- Trip from {} to {} ({} total)",
            times[0],
            times.last().unwrap(),
            *times.last().unwrap() - times[0]
        );
        let mut last_time = times[0];
        for (idx, time) in times.into_iter().enumerate() {
            println!("  Stop {}: {} ({})", idx + 1, time, time - last_time);
            last_time = time;
        }
    }
}
