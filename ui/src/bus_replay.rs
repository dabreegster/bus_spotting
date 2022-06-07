use abstutil::prettyprint_usize;
use geom::{Circle, Distance, Pt2D, Speed, UnitFmt};
use widgetry::mapspace::{ObjectID, World};
use widgetry::{
    Cached, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, State, Text,
    UpdateType, Widget,
};

use model::VehicleID;

use crate::components::{describe, MainMenu, TimeControls};
use crate::{App, Transition};

pub struct BusReplay {
    panel: Panel,
    time_controls: TimeControls,
    world: World<Obj>,
    hover_path: Cached<Obj, Drawable>,
}

impl BusReplay {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = Self {
            panel: crate::components::MainMenu::panel(ctx),
            time_controls: TimeControls::new(ctx, app),
            world: World::unbounded(),
            hover_path: Cached::new(),
        };
        state.on_time_change(ctx, app);
        Box::new(state)
    }

    fn on_time_change(&mut self, ctx: &mut EventCtx, app: &App) {
        let (world, stats) = make_world_and_stats(ctx, app);
        self.world = world;
        self.time_controls.panel.replace(ctx, "stats", stats);
    }
}

impl State<App> for BusReplay {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let prev_time = app.time;
        self.time_controls.event(ctx, app);
        if app.time != prev_time {
            self.on_time_change(ctx, app);
        }

        self.world.event(ctx);

        self.hover_path
            .update(self.world.get_hovering(), |obj| match obj {
                Obj::Bus(id) => {
                    let mut batch = GeomBatch::new();
                    batch.push(
                        Color::CYAN,
                        app.model.vehicles[id.0]
                            .trajectory
                            .as_polyline()
                            .make_polygons(Distance::meters(5.0)),
                    );
                    ctx.upload(batch)
                }
                Obj::Stop(_) => Drawable::empty(ctx),
            });

        if !self.time_controls.is_paused() {
            ctx.request_update(UpdateType::Game);
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if let Some(t) = MainMenu::on_click(ctx, app, x.as_ref()) {
                    return t;
                } else {
                    unreachable!()
                }
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.clear(Color::BLACK);

        self.panel.draw(g);
        self.time_controls.draw(g);
        self.world.draw(g);
        if let Some(draw) = self.hover_path.value() {
            g.redraw(draw);
        }
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    Bus(VehicleID),
    Stop(usize),
}
impl ObjectID for Obj {}

fn make_world_and_stats(ctx: &mut EventCtx, app: &App) -> (World<Obj>, Widget) {
    let mut world = World::bounded(&app.model.bounds);
    // Show the bounds of the world
    world.draw_master_batch(
        ctx,
        GeomBatch::from(vec![(Color::grey(0.1), app.model.bounds.get_rectangle())]),
    );

    // TODO We really need unzoomed circles
    let radius = Distance::meters(50.0);

    // Stops -- these never change; could we avoid rebuilding every time?
    {
        // Optimization
        let circle = Circle::new(Pt2D::zero(), radius).to_polygon();

        for (idx, stop) in app.model.gtfs.stops.values().enumerate() {
            world
                // TODO Need to assign numeric IDs in the model
                .add(Obj::Stop(idx))
                .hitbox(circle.translate(stop.pos.x(), stop.pos.y()))
                .draw_color(Color::BLUE)
                .hover_alpha(0.5)
                .tooltip(describe::stop(stop))
                .build(ctx);
        }
    }

    // Vehicles
    let stats = {
        // TODO UnitFmt::metric()?
        let metric = UnitFmt {
            round_durations: false,
            metric: true,
        };

        let mut away = 0;
        let mut idling = 0;
        let mut moving = 0;

        for vehicle in &app.model.vehicles {
            if let Some((pos, speed)) = vehicle.trajectory.interpolate(app.time) {
                if speed == Speed::ZERO {
                    idling += 1;
                } else {
                    moving += 1;
                }

                world
                    .add(Obj::Bus(vehicle.id))
                    .hitbox(Circle::new(pos, radius).to_polygon())
                    .draw_color(Color::RED)
                    .hover_alpha(0.5)
                    .tooltip(Text::from(format!(
                        "{:?} currently has speed {}",
                        vehicle.original_id,
                        speed.to_string(&metric)
                    )))
                    .build(ctx);
            } else {
                away += 1;
            }
        }
        world.initialize_hover(ctx);

        Text::from_multiline(vec![
            Line(format!("Away: {}", prettyprint_usize(away))),
            Line(format!("Idling: {}", prettyprint_usize(idling))),
            Line(format!("Moving: {}", prettyprint_usize(moving))),
            Line(format!(
                "Stops: {}",
                prettyprint_usize(app.model.gtfs.stops.len())
            )),
        ])
        .into_widget(ctx)
    };

    (world, stats)
}