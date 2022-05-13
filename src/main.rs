#[macro_use]
extern crate anyhow;

mod avl;
mod model;
mod trajectory;

use geom::{Circle, Distance, Time, UnitFmt};
use widgetry::mapspace::{ObjectID, World};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Line, Panel, SharedAppState, State, Text,
    Transition, VerticalAlignment, Widget,
};

use model::{Model, VehicleID, VehicleName};
use trajectory::Trajectory;

fn main() {
    abstutil::logger::setup();

    // TODO Plumb paths
    let model = Model::load("/home/dabreegster/Downloads/mdt_data/AVL/avl_2019-09-01.csv").unwrap();

    widgetry::run(widgetry::Settings::new("Bus Spotting"), move |ctx| {
        let mut app = App {
            model,
            time: Time::START_OF_DAY,
            world: World::unbounded(),
        };
        app.world = make_world(ctx, &app);
        let states = vec![Viewer::new(ctx, &app)];
        (app, states)
    });
}

struct App {
    model: Model,
    time: Time,
    world: World<Obj>,
}

impl SharedAppState for App {}

struct Viewer {
    panel: Panel,
}

impl Viewer {
    fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let bounds = &app.model.bounds;
        ctx.canvas.map_dims = (bounds.max_x, bounds.max_y);
        ctx.canvas.center_on_map_pt(bounds.center());

        Box::new(Self {
            panel: Panel::new_builder(Widget::col(vec![Line("Bus Spotting")
                .small_heading()
                .into_widget(ctx)]))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx),
        })
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        ctx.canvas_movement();

        self.panel.event(ctx);

        app.world.event(ctx);

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(Color::BLACK);

        self.panel.draw(g);
        app.world.draw(g);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    Bus(VehicleID),
}
impl ObjectID for Obj {}

fn make_world(ctx: &mut EventCtx, app: &App) -> World<Obj> {
    let radius = Distance::meters(5.0);
    // TODO UnitFmt::metric()?
    let metric = UnitFmt {
        round_durations: false,
        metric: true,
    };

    let mut world = World::bounded(&app.model.bounds);
    for vehicle in &app.model.vehicles {
        if let Some((pos, speed)) = vehicle.trajectory.interpolate(app.time) {
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
        }
    }
    world.initialize_hover(ctx);
    world
}
