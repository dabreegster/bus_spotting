mod filters;

use geom::{Circle, Distance, Pt2D};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Autocomplete, Color, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, State, Text, Widget,
};

use model::gtfs::{DateFilter, RouteVariantID};

use self::filters::Filters;
use crate::components::{describe, MainMenu};
use crate::{App, Transition};

pub struct Viewer {
    panel: Panel,
    world: World<Obj>,
    filters: Filters,
}

impl Viewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = Self {
            panel: crate::components::MainMenu::panel(ctx),
            world: World::unbounded(),
            filters: Filters::new(),
        };
        state.on_filter_change(ctx, app);
        Box::new(state)
    }

    fn on_filter_change(&mut self, ctx: &mut EventCtx, app: &App) {
        let controls = self.filters.to_controls(ctx, app);
        self.panel.replace(ctx, "contents", controls);

        let variants = if let Some(v) = self.filters.variant {
            vec![v]
        } else {
            app.model
                .gtfs
                .variants_matching_dates(&self.filters.date_filter)
        };
        self.world = make_world(ctx, app, variants);
    }

    // TODO Start a new state for this. Find every variant visiting this stop, show info
    // TODO Also to even put these in the world, we'll need a cheaper stop ID
    fn on_click_stop(&self, _: &mut EventCtx, _: &App, _stop_idx: usize) -> Transition {
        Transition::Keep
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
                        "search for a route variant" => {
                            return Transition::Push(SearchForRouteVariant::new_state(ctx, app));
                        }
                        _ => unreachable!(),
                    }
                }
            }
            Outcome::Changed(x) => {
                // If the user sets an impossible date, this won't run, and the controls will still
                // be fixed at the last valid state
                if let Some(filters) = Filters::from_controls(app, &self.panel) {
                    self.filters = filters;
                    self.on_filter_change(ctx, app);
                }
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

fn make_world(ctx: &mut EventCtx, app: &App, variants: Vec<RouteVariantID>) -> World<Obj> {
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

    // TODO Only stops visited by some route
    for (idx, stop) in app.model.gtfs.stops.values().enumerate() {
        let mut txt = Text::new();
        // TODO Only if we have a variant?
        /*txt.add_line(format!("Stop {}/{}", idx + 1, trip.stop_times.len()));
        txt.add_line(Line(format!("Arrival time: {}", stop_time.arrival_time)));
        txt.add_line(Line(format!(
            "Departure time: {}",
            stop_time.departure_time
        )));*/
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

    /*let mut trip_order_batch = GeomBatch::new();
    for pair in trip.stop_times.windows(2) {
        let stop1 = &app.model.gtfs.stops[&pair[0].stop_id];
        let stop2 = &app.model.gtfs.stops[&pair[1].stop_id];
        trip_order_batch.push(
            Color::RED,
            Line::must_new(stop1.pos, stop2.pos).make_polygons(Distance::meters(20.0)),
        );
    }
    world.draw_master_batch(ctx, trip_order_batch);*/

    world.initialize_hover(ctx);
    world
}

struct SearchForRouteVariant {
    panel: Panel,
}

impl SearchForRouteVariant {
    fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut entries = Vec::new();
        for route in app.model.gtfs.routes.values() {
            for variant in &route.variants {
                entries.push((variant.describe(&app.model.gtfs), variant.variant_id));
            }
        }
        Box::new(Self {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Search for a route variant")
                        .small_heading()
                        .into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Autocomplete::new_widget(ctx, entries, 10).named("search"),
            ]))
            .build(ctx),
        })
    }
}

impl State<App> for SearchForRouteVariant {
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

        if let Some(mut variants) = self.panel.autocomplete_done::<RouteVariantID>("search") {
            if variants.is_empty() {
                return Transition::Pop;
            }
            let variant = variants.remove(0);
            return Transition::Multi(vec![
                Transition::Pop,
                Transition::ModifyState(Box::new(move |state, ctx, app| {
                    let state = state.downcast_mut::<Viewer>().unwrap();
                    state.filters = Filters {
                        date_filter: DateFilter::None,
                        variant: Some(variant),
                    };
                    state.on_filter_change(ctx, app);
                })),
            ]);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}
