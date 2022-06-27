use geom::{Circle, Distance, Pt2D, UnitFmt};
use model::Trajectory;
use widgetry::{
    Cached, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::{App, Transition};

pub struct Compare {
    items: Vec<Item>,
    panel: Panel,
    idx: usize,
    snap_to_trajectory: Cached<Pt2D, (Text, Drawable)>,
}

struct Item {
    trajectory: Trajectory,
    draw: Drawable,
    info: Text,
}

impl Compare {
    pub fn new_state(
        ctx: &mut EventCtx,
        trajectories: Vec<(String, Trajectory)>,
    ) -> Box<dyn State<App>> {
        info!(
            "Setting up trajectory comparison for {} things",
            trajectories.len()
        );

        let mut items: Vec<Item> = Vec::new();
        for (name, trajectory) in trajectories {
            let pl = trajectory.as_polyline();
            let draw = ctx.upload(GeomBatch::from(vec![(
                Color::CYAN,
                pl.make_polygons(Distance::meters(5.0)),
            )]));
            let mut info = Text::from_multiline(vec![
                Line(name),
                Line(format!(
                    "Time range: {} to {}",
                    trajectory.start_time(),
                    trajectory.end_time()
                )),
                Line(format!(
                    "Length: {}",
                    pl.length().to_string(&UnitFmt::metric())
                )),
            ]);
            // Compare everything against the 1st trajectory
            if !items.is_empty() {
                // TODO Slow and poor quality, so disabled
                //let score = items[0].trajectory.score_by_position(&trajectory);
                let score = Distance::ZERO;
                info.add_line(Line(format!(
                    "Diff from 1st: {}",
                    score.to_string(&UnitFmt::metric())
                )));
            }

            items.push(Item {
                trajectory,
                draw,
                info,
            });
        }

        let mut state = Self {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Trajectory debugger").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                // TODO Show buttons for all
                Widget::row(vec![
                    ctx.style()
                        .btn_prev()
                        .hotkey(Key::LeftArrow)
                        .build_widget(ctx, "previous"),
                    Widget::placeholder(ctx, "pointer"),
                    ctx.style()
                        .btn_next()
                        .hotkey(Key::RightArrow)
                        .build_widget(ctx, "next"),
                ])
                .evenly_spaced(),
                Widget::placeholder(ctx, "info"),
                ctx.style()
                    .btn_plain
                    .text("Chop AVL into non-overlapping pieces")
                    .build_def(ctx),
            ]))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx),
            items,
            idx: 0,
            snap_to_trajectory: Cached::new(),
        };

        state.update(ctx);
        Box::new(state)
    }

    fn update(&mut self, ctx: &mut EventCtx) {
        self.panel.replace(
            ctx,
            "pointer",
            format!("{}/{}", self.idx + 1, self.items.len()).text_widget(ctx),
        );

        self.panel.replace(
            ctx,
            "info",
            self.items[self.idx].info.clone().into_widget(ctx),
        );
    }
}

impl State<App> for Compare {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "previous" => {
                    if self.idx != 0 {
                        self.idx -= 1;
                    }
                    self.update(ctx);
                }
                "next" => {
                    if self.idx != self.items.len() - 1 {
                        self.idx += 1;
                    }
                    self.update(ctx);
                }
                "Chop AVL into non-overlapping pieces" => {
                    return Transition::Push(Self::new_state(
                        ctx,
                        self.items[0]
                            .trajectory
                            .split_non_overlapping()
                            .into_iter()
                            .enumerate()
                            .map(|(idx, t)| (format!("AVL piece {}", idx + 1), t))
                            .collect(),
                    ));
                }
                _ => unreachable!(),
            }
        }

        // TODO Refactor parts of this
        self.snap_to_trajectory
            .update(ctx.canvas.get_cursor_in_map_space(), |pt| {
                let mut txt = Text::new();
                let mut batch = GeomBatch::new();
                let hits = self.items[self.idx]
                    .trajectory
                    .times_near_pos(pt, Distance::meters(30.0));
                if !hits.is_empty() {
                    batch.push(
                        Color::YELLOW,
                        Circle::new(hits[0].1, Distance::meters(30.0)).to_polygon(),
                    );
                    let n = hits.len();
                    let first_time = hits[0].0;
                    for (idx, (time, _)) in hits.into_iter().enumerate() {
                        txt.add_line(Line(format!("Here at {time}")));
                        if idx == 4 {
                            txt.append(Line(format!(" (and {} more times)", n - 5)));
                            break;
                        }
                    }

                    // Compare to the AVL
                    if self.idx != 0 {
                        if let Some((pos, _)) = self.items[0].trajectory.interpolate(first_time) {
                            if let Ok(line) = geom::Line::new(pt, pos) {
                                batch.push(Color::PINK, line.make_polygons(Distance::meters(5.0)));
                            }
                        }
                    }
                }
                (txt, ctx.upload(batch))
            });

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        g.redraw(&self.items[self.idx].draw);
        if let Some((txt, draw)) = self.snap_to_trajectory.value() {
            g.redraw(draw);
            g.draw_mouse_tooltip(txt.clone());
        }
    }
}
