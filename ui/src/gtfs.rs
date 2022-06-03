use widgetry::mapspace::{ObjectID, World};
use widgetry::{
    Choice, Color, EventCtx, GfxCtx, Line, Outcome, Panel, State, Text, TextExt, Widget,
};

use model::gtfs::{RouteID, TripID};

use crate::components::MainMenu;
use crate::{App, Transition};

pub struct ViewGTFS {
    panel: Panel,
    route: RouteID,
    trip: TripID,
    world: World<Obj>,
}

impl ViewGTFS {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        // Start with the first route and trip
        let route = app.model.gtfs.routes.values().next().unwrap();
        let trip = route.trips.keys().next().unwrap();

        let mut state = Self {
            panel: crate::components::MainMenu::panel(ctx),
            route: route.route_id.clone(),
            trip: trip.clone(),
            world: World::unbounded(),
        };
        state.on_selection_change(ctx, app);
        Box::new(state)
    }

    fn on_selection_change(&mut self, ctx: &mut EventCtx, app: &App) {
        let route = &app.model.gtfs.routes[&self.route];
        let trip = &route.trips[&self.trip];

        let mut col = vec![Widget::row(vec![
            format!("{} routes", app.model.gtfs.routes.len()).text_widget(ctx),
            Widget::dropdown(
                ctx,
                "route",
                self.route.clone(),
                app.model
                    .gtfs
                    .routes
                    .keys()
                    .map(|r| Choice::new(format!("{:?}", r), r.clone()))
                    .collect(),
            ),
        ])];

        let mut txt = Text::new();
        if let Some(ref x) = route.short_name {
            txt.add_line(Line(format!("Short name: {x}")));
        }
        if let Some(ref x) = route.long_name {
            txt.add_line(Line(format!("Long name: {x}")));
        }
        if let Some(ref x) = route.description {
            txt.add_line(Line(format!("Description: {x}")));
        }
        col.push(txt.into_widget(ctx));

        col.push(Widget::row(vec![
            format!("{} trips", route.trips.len()).text_widget(ctx),
            Widget::dropdown(
                ctx,
                "trip",
                self.trip.clone(),
                route
                    .trips
                    .keys()
                    .map(|t| Choice::new(format!("{:?}", t), t.clone()))
                    .collect(),
            ),
        ]));

        self.panel.replace(ctx, "contents", Widget::col(col));

        // TODO The world
    }
}

impl State<App> for ViewGTFS {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        self.world.event(ctx);

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if let Some(t) = MainMenu::on_click(ctx, app, x.as_ref()) {
                    return t;
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed(x) => {
                match x.as_ref() {
                    "route" => {
                        self.route = self.panel.dropdown_value("route");
                        self.trip = app.model.gtfs.routes[&self.route]
                            .trips
                            .keys()
                            .next()
                            .unwrap()
                            .clone();
                    }
                    "trip" => {
                        self.trip = self.panel.dropdown_value("trip");
                    }
                    _ => unreachable!(),
                }
                self.on_selection_change(ctx, app);
            }
            _ => {}
        }

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
    Stop(usize),
}
impl ObjectID for Obj {}
