#[macro_use]
extern crate anyhow;

mod avl;
mod model;
mod trajectory;

use abstutil::prettyprint_usize;
use geom::{Circle, Distance, Duration, Speed, Time, UnitFmt};
use widgetry::mapspace::{ObjectID, World};
use widgetry::{
    Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, SharedAppState,
    Slider, State, Text, Transition, VerticalAlignment, Widget,
};

use model::{Model, VehicleID, VehicleName};
use trajectory::Trajectory;

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
        };
        let states = vec![Viewer::new(ctx, &app)];
        (app, states)
    });
}

struct App {
    model: Model,
    time: Time,
}

impl SharedAppState for App {}

struct Viewer {
    panel: Panel,
    world: World<Obj>,
}

impl Viewer {
    fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = Self {
            panel: Panel::new_builder(Widget::col(vec![
                Line("Bus Spotting").small_heading().into_widget(ctx),
                Slider::area(
                    ctx,
                    0.15 * ctx.canvas.window_width,
                    app.time.to_percent(end_of_day()),
                    "time",
                ),
                // TODO Widget::placeholder()?
                Text::new().into_widget(ctx).named("controls"),
            ]))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx),
            world: World::unbounded(),
        };
        state.on_time_change(ctx, app);
        Box::new(state)
    }

    fn on_time_change(&mut self, ctx: &mut EventCtx, app: &App) {
        let (world, controls) = make_world_and_panel(ctx, app);
        self.world = world;
        self.panel.replace(ctx, "controls", controls);
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Changed(_) => {
                app.time = end_of_day().percent_of(self.panel.slider("time").get_percent());
                self.on_time_change(ctx, app);
            }
            _ => {}
        }

        self.world.event(ctx);

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.clear(Color::BLACK);

        self.panel.draw(g);
        self.world.draw(g);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    Bus(VehicleID),
}
impl ObjectID for Obj {}

fn make_world_and_panel(ctx: &mut EventCtx, app: &App) -> (World<Obj>, Widget) {
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

    let controls = Text::from_multiline(vec![
        Line(format!("Time: {}", app.time)),
        Line(format!("Away: {}", prettyprint_usize(away))),
        Line(format!("Idling: {}", prettyprint_usize(idling))),
        Line(format!("Moving: {}", prettyprint_usize(moving))),
    ])
    .into_widget(ctx);

    (world, controls)
}

fn end_of_day() -> Time {
    Time::START_OF_DAY + Duration::hours(24)
}
