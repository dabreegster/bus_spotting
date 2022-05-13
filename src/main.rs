#[macro_use]
extern crate anyhow;

mod model;
mod speed;

use abstutil::prettyprint_usize;
use geom::{Circle, Distance, Duration, Speed, Time, UnitFmt};
use widgetry::mapspace::{ObjectID, World};
use widgetry::{
    Cached, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, SharedAppState, State, Text,
    Transition, UpdateType, Widget,
};

use self::model::{Model, VehicleID};
use self::speed::TimeControls;

fn main() {
    abstutil::logger::setup();

    // TODO Plumb paths
    let model = Model::load("/home/dabreegster/Downloads/mdt_data/AVL/avl_2019-09-01.csv").unwrap();

    widgetry::run(widgetry::Settings::new("Bus Spotting"), move |ctx| {
        let bounds = &model.bounds;
        ctx.canvas.map_dims = (bounds.max_x, bounds.max_y);
        ctx.canvas.center_on_map_pt(bounds.center());

        let app = App {
            model,
            time: Time::START_OF_DAY,
            time_increment: Duration::minutes(10),
        };
        let states = vec![Viewer::new(ctx, &app)];
        (app, states)
    });
}

pub struct App {
    model: Model,
    time: Time,
    time_increment: Duration,
}

impl SharedAppState for App {}

struct Viewer {
    time_controls: TimeControls,
    world: World<Obj>,
    hover_path: Cached<Obj, Drawable>,
}

impl Viewer {
    fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = Self {
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

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        ctx.canvas_movement();

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
            });

        if !self.time_controls.is_paused() {
            ctx.request_update(UpdateType::Game);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.clear(Color::BLACK);

        self.time_controls.draw(g);
        self.world.draw(g);
        if let Some(draw) = self.hover_path.value() {
            g.redraw(draw);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    Bus(VehicleID),
}
impl ObjectID for Obj {}

fn make_world_and_stats(ctx: &mut EventCtx, app: &App) -> (World<Obj>, Widget) {
    // TODO We really need unzoomed circles
    let radius = Distance::meters(50.0);
    // TODO UnitFmt::metric()?
    let metric = UnitFmt {
        round_durations: false,
        metric: true,
    };

    let mut world = World::bounded(&app.model.bounds);
    // Show the bounds of the world
    world.draw_master_batch(
        ctx,
        GeomBatch::from(vec![(Color::grey(0.1), app.model.bounds.get_rectangle())]),
    );

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

    let stats = Text::from_multiline(vec![
        Line(format!("Away: {}", prettyprint_usize(away))),
        Line(format!("Idling: {}", prettyprint_usize(idling))),
        Line(format!("Moving: {}", prettyprint_usize(moving))),
    ])
    .into_widget(ctx);

    (world, stats)
}
