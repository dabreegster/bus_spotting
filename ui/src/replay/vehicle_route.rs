use geom::{Circle, Distance, Pt2D};
use model::gtfs::RouteVariantID;
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
            draw.push(Color::RED, pl.make_polygons(Distance::meters(3.0)));
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
