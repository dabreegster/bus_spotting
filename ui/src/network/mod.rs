mod filters;
mod search;

use std::collections::BTreeSet;

use geom::{Circle, Distance, Pt2D};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{Color, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, State, Text};

use model::gtfs::{RouteVariantID, StopID};

pub use self::filters::Filters;
use crate::components::{describe, MainMenu};
use crate::{App, Transition};

pub struct Viewer {
    panel: Panel,
    world: World<Obj>,
}

impl Viewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = Self {
            panel: crate::components::MainMenu::panel(ctx, crate::components::Mode::Network),
            world: World::unbounded(),
        };
        state.on_filter_change(ctx, app);
        Box::new(state)
    }

    // TODO Could move to App
    fn selected_variants(&self, app: &App) -> BTreeSet<RouteVariantID> {
        if let Some(v) = app.filters.variant {
            vec![v].into_iter().collect()
        } else {
            app.model
                .gtfs
                .variants_matching_dates(&app.filters.date_filter)
        }
    }

    fn on_filter_change(&mut self, ctx: &mut EventCtx, app: &App) {
        let controls = app.filters.to_controls(ctx, app);
        self.panel.replace(ctx, "contents", controls);

        let world = make_world(ctx, app, self.selected_variants(app));
        self.world = world;
    }

    fn on_click_stop(&self, ctx: &mut EventCtx, app: &App, stop_id: StopID) -> Transition {
        let stop = &app.model.gtfs.stops[&stop_id];

        let variants = stop
            .route_variants
            .intersection(&self.selected_variants(app))
            .cloned()
            .collect::<Vec<RouteVariantID>>();

        let first = variants[0];
        Transition::Push(crate::stop::StopInfo::new_state(
            ctx, app, stop, variants, first,
        ))
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let WorldOutcome::ClickedObject(Obj::Stop(id)) = self.world.event(ctx) {
            return self.on_click_stop(ctx, app, id);
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
                                    .variants_matching_dates(&app.filters.date_filter),
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
                    app.filters = filters;
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
    Stop(StopID),
    Route(RouteVariantID),
}
impl ObjectID for Obj {}

fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    selected_variants: BTreeSet<RouteVariantID>,
) -> World<Obj> {
    let mut world = World::bounded(&app.model.bounds);
    // Show the bounds of the world
    world.draw_master_batch(
        ctx,
        GeomBatch::from(vec![(Color::grey(0.1), app.model.bounds.get_rectangle())]),
    );

    // Draw every route variant. Track what stops we visit
    let mut stops: BTreeSet<StopID> = BTreeSet::new();
    for id in &selected_variants {
        let variant = app.model.gtfs.variant(*id);
        for stop_time in &variant.trips[0].stop_times {
            stops.insert(stop_time.stop_id);
        }

        if let Ok(pl) = variant.polyline(&app.model.gtfs) {
            let mut txt = Text::new();
            txt.add_line(Line(variant.describe(&app.model.gtfs)));

            // TODO Most variants overlap. Maybe perturb the lines a bit, or use the overlapping
            // path trick
            world
                .add(Obj::Route(*id))
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
    for id in stops {
        let stop = &app.model.gtfs.stops[&id];
        let mut txt = describe::stop(stop);
        txt.add_line(format!(
            "{} route variants",
            stop.route_variants
                .intersection(&selected_variants)
                .collect::<Vec<_>>()
                .len()
        ));

        world
            .add(Obj::Stop(id))
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
