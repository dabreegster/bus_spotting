mod avl;
mod model;
mod trajectory;

use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Line, Panel, SharedAppState, State, Transition,
    VerticalAlignment, Widget,
};

use model::{Model, VehicleID};
use trajectory::Trajectory;

fn main() {
    abstutil::logger::setup();

    // TODO Plumb paths
    let model = Model::load("/home/dabreegster/Downloads/mdt_data/AVL/avl_2019-09-01.csv").unwrap();

    widgetry::run(widgetry::Settings::new("Bus Spotting"), move |ctx| {
        let app = App { model };
        let states = vec![Viewer::new(ctx, &app)];
        (app, states)
    });
}

struct App {
    model: Model,
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
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition<App> {
        ctx.canvas_movement();

        self.panel.event(ctx);

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.clear(Color::BLACK);

        self.panel.draw(g);
    }
}
