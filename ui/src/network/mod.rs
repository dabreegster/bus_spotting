mod filters;

use geom::{Circle, Distance, Line, Pt2D};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::tools::PopupMsg;
use widgetry::{
    include_labeled_bytes, lctrl, Autocomplete, Choice, Color, EventCtx, GeomBatch, GfxCtx, Key,
    Line, Outcome, Panel, State, Text, TextExt, Widget,
};

use model::gtfs::{DateFilter, RouteID, RouteVariantID, Trip, TripID};

use crate::components::{date_filter, describe, MainMenu};
use crate::{App, Transition};

pub struct Viewer {
    panel: Panel,
    world: World<Obj>,
    date_filter: DateFilter,
    route: RouteID,
    variant: Option<RouteVariantID>,
    trip: TripID,
}

impl Viewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        // TODO Slight hack. If we're on an empty model, this viewer will crash, so just redirect
        // to the other mode for now.
        if app.model.gtfs.routes.is_empty() {
            return crate::bus_replay::BusReplay::new_state(ctx, app);
        }

        // Start with the first route and trip
        let route = app.model.gtfs.routes.values().next().unwrap();
        let trip = route.trips.keys().next().unwrap();

        let mut state = Self {
            panel: crate::components::MainMenu::panel(ctx),
            date_filter: DateFilter::None,
            route: route.route_id.clone(),
            trip: trip.clone(),
            variant: None,
            world: World::unbounded(),
        };
        state.on_selection_change(ctx, app);
        Box::new(state)
    }

    fn on_selection_change(&mut self, ctx: &mut EventCtx, app: &App) {
        let route = &app.model.gtfs.routes[&self.route];
        let trip = &route.trips[&self.trip];

        let mut col = Vec::new();

        col.push(date_filter::to_controls(ctx, &self.date_filter).section(ctx));

        col.push(Widget::row(vec![
            format!("{} routes", app.model.gtfs.routes.len()).text_widget(ctx),
            Widget::dropdown(
                ctx,
                "route",
                self.route.clone(),
                app.model
                    .gtfs
                    .routes
                    .values()
                    .map(|r| {
                        Choice::new(
                            format!("{:?} - {}", r.route_id, r.describe()),
                            r.route_id.clone(),
                        )
                    })
                    .collect(),
            ),
            ctx.style()
                .btn_plain
                .icon_bytes(include_labeled_bytes!("../../assets/search.svg"))
                .hotkey(lctrl(Key::F))
                .build_widget(ctx, "search for a route"),
        ]));

        col.push(describe::route(route).into_widget(ctx));

        let mut variant_choices = vec![Choice::new("no variant / all trips", None)];
        for v in &route.variants {
            let name = match v.headsign {
                Some(ref x) => format!("{:?} ({x})", v.variant_id),
                None => format!("{:?}", v.variant_id),
            };
            variant_choices.push(Choice::new(
                format!(
                    "{} - {}, {} trips",
                    name,
                    app.model.gtfs.calendar.services[&v.service_id]
                        .days_of_week
                        .describe(),
                    v.trips.len()
                ),
                Some(v.variant_id),
            ));
        }
        col.push(Widget::row(vec![
            format!("{} variants", route.variants.len()).text_widget(ctx),
            Widget::dropdown(ctx, "variant", self.variant, variant_choices),
        ]));

        // TODO Can we avoid the cloning?
        let filtered_trips = if let Some(variant) = self.variant {
            route.variants[variant.0].trips.clone()
        } else {
            route.trips.keys().cloned().collect()
        };
        col.push(Widget::row(vec![
            format!("{} trips", filtered_trips.len()).text_widget(ctx),
            Widget::dropdown(
                ctx,
                "trip",
                self.trip.clone(),
                filtered_trips
                    .into_iter()
                    .map(|t| {
                        Choice::new(
                            format!(
                                "{:?} - starting {}",
                                t, route.trips[&t].stop_times[0].arrival_time
                            ),
                            t,
                        )
                    })
                    .collect(),
            ),
        ]));
        if let Some(ref x) = trip.headsign {
            col.push(format!("Headsign: {x}").text_widget(ctx));
        }
        col.push(
            describe::service(&app.model.gtfs.calendar.services[&trip.service_id]).into_widget(ctx),
        );

        self.panel
            .replace(ctx, "contents", Widget::col(col).section(ctx));

        self.world = make_world(ctx, app, trip);
    }

    fn on_click_stop(&self, ctx: &mut EventCtx, app: &App, stop_idx: usize) -> Transition {
        let route = &app.model.gtfs.routes[&self.route];
        let variant = if let Some(variant) = self.variant {
            &route.variants[variant.0]
        } else {
            return Transition::Keep;
        };
        let stop_id = &route.trips[&self.trip].stop_times[stop_idx].stop_id;

        // Show the schedule for this stop
        let mut txt = Text::new();
        txt.add_line(Line(format!("Schedule for route {}", route.describe())).small_heading());
        txt.extend(describe::stop(&app.model.gtfs.stops[stop_id]));
        txt.add_line(Line(""));
        for trip in &variant.trips {
            let trip = &route.trips[trip];
            txt.add_line(Line(trip.arrival_at(stop_id).to_string()));
            if trip.trip_id == self.trip {
                txt.append(Line(" (current trip)"));
            }
        }

        // TODO The world tooltip sticks around, oops
        Transition::Push(PopupMsg::new_state_for_txt(ctx, txt))
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let WorldOutcome::ClickedObject(Obj::Stop(idx)) = self.world.event(ctx) {
            return self.on_click_stop(ctx, app, idx);
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if let Some(t) = MainMenu::on_click(ctx, app, x.as_ref()) {
                    return t;
                } else {
                    match x.as_ref() {
                        "search for a route" => {
                            return Transition::Push(SearchForRoute::new_state(ctx, app));
                        }
                        _ => unreachable!(),
                    }
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
                        self.variant = None;
                    }
                    "variant" => {
                        self.variant = self.panel.dropdown_value("variant");
                        let route = &app.model.gtfs.routes[&self.route];
                        self.trip = match self.variant {
                            Some(variant) => route.variants[variant.0].trips[0].clone(),
                            None => route.trips.keys().next().unwrap().clone(),
                        };
                    }
                    "trip" => {
                        self.trip = self.panel.dropdown_value("trip");
                    }
                    _ => {
                        // If the user sets an impossible date, this won't run, and the controls
                        // will still be fixed at the last valid state
                        if let Some(filter) = date_filter::from_controls(&self.panel) {
                            self.date_filter = filter;
                        }
                        // TODO Reset everything else...
                    }
                }
                self.on_selection_change(ctx, app);
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.world.draw(g);
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app)
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
        txt.extend(describe::stop(stop));

        world
            .add(Obj::Stop(idx))
            .hitbox(circle.translate(stop.pos.x(), stop.pos.y()))
            .draw_color(Color::BLUE)
            .hover_alpha(0.5)
            .tooltip(txt)
            .clickable()
            .build(ctx);
    }

    let mut trip_order_batch = GeomBatch::new();
    for pair in trip.stop_times.windows(2) {
        let stop1 = &app.model.gtfs.stops[&pair[0].stop_id];
        let stop2 = &app.model.gtfs.stops[&pair[1].stop_id];
        trip_order_batch.push(
            Color::RED,
            Line::must_new(stop1.pos, stop2.pos).make_polygons(Distance::meters(20.0)),
        );
    }
    world.draw_master_batch(ctx, trip_order_batch);

    world.initialize_hover(ctx);
    world
}

struct SearchForRoute {
    panel: Panel,
}

impl SearchForRoute {
    fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut entries = Vec::new();
        for route in app.model.gtfs.routes.values() {
            entries.push((route.describe(), route.route_id.clone()));
        }
        Box::new(Self {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Search for a route").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Autocomplete::new_widget(ctx, entries, 10).named("search"),
            ]))
            .build(ctx),
        })
    }
}

impl State<App> for SearchForRoute {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        if let Some(mut routes) = self.panel.autocomplete_done::<RouteID>("search") {
            if routes.is_empty() {
                return Transition::Pop;
            }
            let route = routes.remove(0);
            return Transition::Multi(vec![
                Transition::Pop,
                Transition::ModifyState(Box::new(move |state, ctx, app| {
                    let state = state.downcast_mut::<Viewer>().unwrap();
                    state.route = route;
                    state.trip = app.model.gtfs.routes[&state.route]
                        .trips
                        .keys()
                        .next()
                        .unwrap()
                        .clone();
                    state.variant = None;
                    state.on_selection_change(ctx, app);
                })),
            ]);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}
