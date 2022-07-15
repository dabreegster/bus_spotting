mod filters;
mod search;
mod stop;
mod variant;

use std::collections::BTreeSet;

use abstutil::Timer;
use geom::{Circle, Distance, Pt2D};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{Color, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, State, Text};

use gtfs::{RouteVariantID, StopID};

use self::filters::Filters;
use crate::components::{describe, MainMenu};

pub struct Viewer {
    panel: Panel,
    world: World<Obj>,
}

impl Viewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = Self {
            panel: crate::components::MainMenu::panel(ctx),
            world: World::unbounded(),
        };
        state.on_filter_change(ctx, app);
        Box::new(state)
    }

    fn on_filter_change(&mut self, ctx: &mut EventCtx, app: &App) {
        ctx.loading_screen("update filters", |ctx, timer| {
            let controls = app.filters.to_controls(ctx, app);
            self.panel.replace(ctx, "contents", controls);

            let world = make_world(ctx, app, timer);
            self.world = world;
        });
    }

    fn on_click_stop(&self, ctx: &mut EventCtx, app: &App, stop_id: StopID) -> Transition {
        let stop = &app.model.gtfs.stops[&stop_id];

        let variants = stop
            .route_variants
            .intersection(&app.filters.selected_variants(app))
            .cloned()
            .collect::<Vec<RouteVariantID>>();

        let first = variants[0];
        Transition::Push(stop::StopInfo::new_state(ctx, app, stop, variants, first))
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        app.sync_mapbox(ctx);

        match self.world.event(ctx) {
            WorldOutcome::ClickedObject(Obj::Stop(id)) => {
                self.world.hack_unset_hovering();
                return self.on_click_stop(ctx, app, id);
            }
            WorldOutcome::ClickedObject(Obj::Route(id)) => {
                self.world.hack_unset_hovering();
                return Transition::Push(variant::VariantInfo::new_state(
                    ctx,
                    app,
                    app.model.gtfs.variant(id),
                ));
            }
            _ => {}
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if let Some(t) = MainMenu::on_click_network(ctx, x.as_ref()) {
                    return t;
                } else {
                    match x.as_ref() {
                        "search for a route variant" => {
                            return Transition::Push(search::SearchForRouteVariant::new_state(
                                ctx,
                                app,
                                app.model.gtfs.variants_matching_filter(&app.filters.filter),
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

fn make_world(ctx: &mut EventCtx, app: &App, timer: &mut Timer) -> World<Obj> {
    let selected_variants = app.filters.selected_variants(app);
    let mut world = World::bounded(&app.model.bounds);

    // Draw every route variant. Track what stops we visit
    let mut stops: BTreeSet<StopID> = BTreeSet::new();
    timer.start_iter("draw variants", selected_variants.len());
    for id in &selected_variants {
        timer.next();
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
                // Since they overlap, no use wasting memory here for an effect
                .invisibly_hoverable()
                .tooltip(txt)
                .clickable()
                .build(ctx);
        }
    }

    // TODO We really need unzoomed circles
    let radius = Distance::meters(50.0);
    // Optimization
    let circle = Circle::new(Pt2D::zero(), radius).to_polygon();
    let circle_outline = Circle::new(Pt2D::zero(), radius)
        .to_outline(Distance::meters(3.0))
        .unwrap();

    // Only draw visited stops
    timer.start_iter("draw stops", stops.len());
    for id in stops {
        timer.next();
        let stop = &app.model.gtfs.stops[&id];
        let mut txt = describe::stop(stop);
        txt.add_line(format!(
            "{} route variants",
            stop.route_variants
                .intersection(&selected_variants)
                .collect::<Vec<_>>()
                .len()
        ));

        let hitbox = circle.translate(stop.pos.x(), stop.pos.y());
        let mut batch = GeomBatch::new();
        batch.push(Color::BLUE, hitbox.clone());
        batch.push(
            Color::WHITE,
            circle_outline.translate(stop.pos.x(), stop.pos.y()),
        );

        world
            .add(Obj::Stop(id))
            .hitbox(hitbox)
            .draw(batch)
            .hover_alpha(0.5)
            .tooltip(txt)
            .clickable()
            .build(ctx);
    }

    world.initialize_hover(ctx);
    world
}

// TODO Split files

use geom::Bounds;
use model::MultidayModel;
use serde::{Deserialize, Serialize};
use widgetry::{Canvas, SharedAppState};

pub struct App {
    model: MultidayModel,

    filters: Filters,

    // Avoid syncing when bounds match
    #[allow(unused)]
    mapbox_bounds: Bounds,
}

impl SharedAppState for App {
    fn draw_default(&self, g: &mut GfxCtx) {
        if cfg!(not(target_arch = "wasm32")) {
            g.clear(Color::BLACK);
        }
    }

    fn before_quit(&self, canvas: &Canvas) {
        let ss = Savestate {
            cam_x: canvas.cam_x,
            cam_y: canvas.cam_y,
            cam_zoom: canvas.cam_zoom,
        };
        abstio::write_json("data/save_network.json".to_string(), &ss);
    }
}

pub type Transition = widgetry::Transition<App>;

impl App {
    pub fn new(ctx: &mut EventCtx, model: MultidayModel) -> Self {
        let bounds = &model.bounds;
        ctx.canvas.map_dims = (bounds.max_x, bounds.max_y);
        ctx.canvas.center_on_map_pt(bounds.center());

        Self {
            model,

            filters: Filters::new(),

            mapbox_bounds: Bounds::new(),
        }
    }

    #[allow(unused)]
    pub fn sync_mapbox(&mut self, ctx: &mut EventCtx) {
        #[cfg(target_arch = "wasm32")]
        {
            // This method is usually called for every single event, but the camera hasn't always
            // moved
            let bounds = ctx.canvas.get_screen_bounds();
            if self.mapbox_bounds == bounds {
                return;
            }
            self.mapbox_bounds = bounds;

            let pt1 = geom::Pt2D::new(bounds.min_x, bounds.min_y).to_gps(&self.model.gps_bounds);
            let pt2 = geom::Pt2D::new(bounds.max_x, bounds.max_y).to_gps(&self.model.gps_bounds);
            sync_mapbox_canvas(pt1.x(), pt1.y(), pt2.x(), pt2.y());
        }
    }

    // This only makes sense on native, with the same model used across different runs.
    // before_quit is never called on web, and web starts with an empty model.
    pub fn restore_savestate(&self, ctx: &mut EventCtx) {
        if let Ok(savestate) = abstio::maybe_read_json::<Savestate>(
            "data/save.json".to_string(),
            &mut Timer::throwaway(),
        ) {
            ctx.canvas.cam_x = savestate.cam_x;
            ctx.canvas.cam_y = savestate.cam_y;
            ctx.canvas.cam_zoom = savestate.cam_zoom;
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Savestate {
    cam_x: f64,
    cam_y: f64,
    cam_zoom: f64,
}
