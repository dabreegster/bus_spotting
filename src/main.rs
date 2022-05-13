use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Line, Panel, SharedAppState, State, Transition,
    VerticalAlignment, Widget,
};

fn main() {
    abstutil::logger::setup();
    widgetry::run(widgetry::Settings::new("Bus Spotting"), |ctx| {
        let app = App {};
        let states = vec![Viewer::new(ctx, &app)];
        (app, states)
    });
}

struct App {}

impl SharedAppState for App {}

struct Viewer {
    panel: Panel,
}

impl Viewer {
    fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        /*let bounds = model.get_bounds();
        ctx.canvas.map_dims = (bounds.max_x, bounds.max_y);
        ctx.canvas.center_on_map_pt(bounds.center());*/

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

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.clear(Color::BLACK);

        self.panel.draw(g);
    }
}
