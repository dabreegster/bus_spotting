mod filters;
mod search;

use std::collections::BTreeSet;

use geom::{Circle, Distance, PolyLine, Pt2D};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{Color, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, State, Text};

use model::gtfs::{RouteVariantID, StopID};

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
                            return Transition::Push(search::SearchForRouteVariant::new_state(
                                ctx,
                                app,
                                app.model
                                    .gtfs
                                    .variants_matching_dates(&self.filters.date_filter),
                            ));
                        }
                        _ => unreachable!(),
                    }
                }
            }
            Outcome::Changed(_x) => {
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
    Route(RouteVariantID),
}
impl ObjectID for Obj {}

fn make_world(ctx: &mut EventCtx, app: &App, variants: Vec<RouteVariantID>) -> World<Obj> {
    let mut world = World::bounded(&app.model.bounds);
    // Show the bounds of the world
    world.draw_master_batch(
        ctx,
        GeomBatch::from(vec![(Color::grey(0.1), app.model.bounds.get_rectangle())]),
    );

    // Draw every route variant. Track what stops we visit
    let mut stops: BTreeSet<&StopID> = BTreeSet::new();
    for id in variants {
        let variant = app.model.gtfs.variant(id);
        let trip = &app.model.gtfs.routes[&variant.route_id].trips[&variant.trips[0]];
        let mut pts = Vec::new();
        for stop_time in &trip.stop_times {
            let stop = &app.model.gtfs.stops[&stop_time.stop_id];
            pts.push(stop.pos);
            stops.insert(&stop.stop_id);
        }

        if let Ok(pl) = PolyLine::new(pts) {
            let mut txt = Text::new();
            txt.add_line(Line(variant.describe(&app.model.gtfs)));

            // TODO Most variants overlap. Maybe perturb the lines a bit, or use the overlapping
            // path trick
            world
                .add(Obj::Route(id))
                .hitbox(pl.make_polygons(Distance::meters(20.0)))
                .draw_color(Color::RED)
                .hover_alpha(0.5)
                .tooltip(txt)
                .clickable()
                .build(ctx);
        }
    }

    // TODO We really need unzoomed circles
    let radius = Distance::meters(50.0);
    // Optimization
    let circle = Circle::new(Pt2D::zero(), radius).to_polygon();

    // Only draw visited stops
    for (idx, id) in stops.into_iter().enumerate() {
        let stop = &app.model.gtfs.stops[id];
        let txt = describe::stop(stop);

        world
            .add(Obj::Stop(idx))
            .hitbox(circle.translate(stop.pos.x(), stop.pos.y()))
            .draw_color(Color::BLUE)
            .hover_alpha(0.5)
            .tooltip(txt)
            .clickable()
            .build(ctx);
    }

    world.initialize_hover(ctx);
    world
}
