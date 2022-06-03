use geom::{Circle, Distance, Pt2D};
use widgetry::mapspace::{ObjectID, World};
use widgetry::{
    Choice, Color, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, State, Text, TextExt, Widget,
};

use model::gtfs::{RouteID, Trip, TripID};

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

        self.world = make_world(ctx, app, trip);
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

fn make_world(ctx: &mut EventCtx, app: &App, trip: &Trip) -> World<Obj> {
    let mut world = World::bounded(&app.model.bounds);
    // Show the bounds of the world
    world.draw_master_batch(
        ctx,
        GeomBatch::from(vec![(Color::grey(0.1), app.model.bounds.get_rectangle())]),
    );

    // TODO We really need unzoomed circles
    let radius = Distance::meters(50.0);
    // Optimization
    let circle = Circle::new(Pt2D::zero(), radius).to_polygon();

    for (idx, stop_time) in trip.stop_times.iter().enumerate() {
        let stop = &app.model.gtfs.stops[&stop_time.stop_id];

        let mut txt = Text::new();
        txt.add_line(format!("Stop {}/{}", idx + 1, trip.stop_times.len()));
        txt.add_line(Line(format!("Arrival time: {}", stop_time.arrival_time)));
        txt.add_line(Line(format!(
            "Departure time: {}",
            stop_time.departure_time
        )));
        // TODO Share with other tool
        txt.add_line(format!("{:?}", stop.stop_id));
        if let Some(ref name) = stop.name {
            txt.add_line(Line(format!("Name: {name}")));
        }
        if let Some(ref code) = stop.code {
            txt.add_line(Line(format!("Code: {code}")));
        }
        if let Some(ref description) = stop.description {
            txt.add_line(Line(format!("Description: {description}")));
        }

        world
            .add(Obj::Stop(idx))
            .hitbox(circle.translate(stop.pos.x(), stop.pos.y()))
            .draw_color(Color::BLUE)
            .hover_alpha(0.5)
            .tooltip(txt)
            .build(ctx);
    }

    world.initialize_hover(ctx);
    world
}
