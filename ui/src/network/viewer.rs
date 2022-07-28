use std::collections::BTreeSet;

use abstutil::{prettyprint_usize, Counter, Timer};
use geom::{Circle, Distance, Pt2D};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::tools::{ColorLegend, ColorScale};
use widgetry::{
    Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, State, Text,
    TextExt, Toggle, Widget,
};

use gtfs::{RouteVariantID, StopID};

use super::{App, Filters, Transition};
use crate::components::{describe, MainMenu};

pub struct Viewer {
    panel: Panel,
    world: World<Obj>,
    draw_streets: Drawable,
}

impl Viewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = Self {
            panel: crate::components::MainMenu::panel(ctx),
            world: World::unbounded(),
            draw_streets: Drawable::empty(ctx),
        };

        let mut batch = GeomBatch::new();
        batch.extend(Color::grey(0.5), app.model.gtfs.road_geometry.clone());
        batch.extend(
            Color::grey(0.7),
            app.model.gtfs.intersection_geometry.clone(),
        );
        state.draw_streets = ctx.upload(batch);

        state.on_filter_change(ctx, app);
        Box::new(state)
    }

    pub fn on_filter_change(&mut self, ctx: &mut EventCtx, app: &App) {
        ctx.loading_screen("update filters", |ctx, timer| {
            let controls = Widget::col(vec![
                app.filters.to_controls(ctx, app),
                Widget::row(vec![
                    "Show stops:".text_widget(ctx),
                    // Weird pattern
                    Widget::dropdown(
                        ctx,
                        "stop style",
                        self.panel
                            .maybe_dropdown_value("stop style")
                            .unwrap_or(StopStyle::None),
                        vec![
                            Choice::new("all", StopStyle::None),
                            Choice::new("by boardings", StopStyle::Boardings),
                            Choice::new("daily trips (any variant)", StopStyle::NumberTrips),
                        ],
                    ),
                ]),
                Toggle::choice(
                    ctx,
                    "draw routes",
                    "snapped",
                    "original",
                    Key::S,
                    self.panel.maybe_is_checked("draw routes").unwrap_or(false),
                ),
                Widget::placeholder(ctx, "stop style info"),
                ctx.style()
                    .btn_outline
                    .text("Boardings by variant")
                    .build_def(ctx),
                ctx.style().btn_outline.text("Export to CSV").build_def(ctx),
            ]);
            self.panel.replace(ctx, "contents", controls);

            let world = make_world(ctx, app, &mut self.panel, timer);
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
        Transition::Push(super::stop::StopInfo::new_state(
            ctx, app, stop, variants, first,
        ))
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        app.mapbox.sync(ctx, &app.model.gps_bounds);

        match self.world.event(ctx) {
            WorldOutcome::ClickedObject(Obj::Stop(id)) => {
                self.world.hack_unset_hovering();
                return self.on_click_stop(ctx, app, id);
            }
            WorldOutcome::ClickedObject(Obj::Route(id)) => {
                self.world.hack_unset_hovering();
                return Transition::Push(super::variant::VariantInfo::new_state(
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
                            return Transition::Push(
                                super::search::SearchForRouteVariant::new_state(
                                    ctx,
                                    app,
                                    app.model.gtfs.variants_matching_filter(&app.filters.filter),
                                ),
                            );
                        }
                        "Boardings by variant" => {
                            return Transition::Push(
                                super::analysis::Analysis::boardings_by_variant(ctx, app),
                            );
                        }
                        "Export to CSV" => {
                            abstio::write_file(
                                "multiday.csv".to_string(),
                                app.model.export_to_csv().unwrap(),
                            )
                            .unwrap();
                        }
                        _ => unreachable!(),
                    }
                }
            }
            Outcome::Changed(_x) => {
                // This also happens now for StopStyle and draw routes

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
        g.redraw(&self.draw_streets);
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

fn make_world(ctx: &mut EventCtx, app: &App, panel: &mut Panel, timer: &mut Timer) -> World<Obj> {
    let selected_variants = app.filters.selected_variants(app);
    let mut world = World::bounded(&app.model.bounds);

    // Draw every route variant. Track what stops we visit
    let draw_snapped_routes = panel.is_checked("draw routes");
    let mut stops: BTreeSet<StopID> = BTreeSet::new();
    timer.start_iter("draw variants", selected_variants.len());
    for id in &selected_variants {
        timer.next();
        let variant = app.model.gtfs.variant(*id);
        for stop_time in &variant.trips[0].stop_times {
            stops.insert(stop_time.stop_id);
        }

        if let Ok(pl) = variant.maybe_snapped_polyline(&app.model.gtfs, draw_snapped_routes) {
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

    match panel.dropdown_value("stop style") {
        StopStyle::None => {
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
        }
        StopStyle::Boardings => {
            let mut counts = app.model.count_boardings_by_stop();
            counts.subset(&stops);
            heatmap_stops(
                ctx,
                app,
                panel,
                &mut world,
                counts,
                "total boardings",
                timer,
            );
        }
        StopStyle::NumberTrips => {
            let counts = count_daily_trips_per_stop(app, &stops, &selected_variants);
            heatmap_stops(ctx, app, panel, &mut world, counts, "daily trips", timer);
        }
    }

    world.initialize_hover(ctx);
    world
}

#[derive(Clone, Debug, PartialEq)]
enum StopStyle {
    None,
    Boardings,
    NumberTrips,
    // frequency
}

fn count_daily_trips_per_stop(
    app: &App,
    stops: &BTreeSet<StopID>,
    selected_variants: &BTreeSet<RouteVariantID>,
) -> Counter<StopID> {
    let mut cnt = Counter::new();
    for stop in stops {
        let mut trips = 0;
        for variant in app.model.gtfs.stops[stop]
            .route_variants
            .intersection(selected_variants)
        {
            trips += app.model.gtfs.variant(*variant).trips.len();
        }
        cnt.add(*stop, trips);
    }
    cnt
}

fn heatmap_stops(
    ctx: &mut EventCtx,
    app: &App,
    panel: &mut Panel,
    world: &mut World<Obj>,
    counts: Counter<StopID>,
    label: &str,
    timer: &mut Timer,
) {
    let max = counts.max() as f64;
    let scale = ColorScale::from_colorous(colorous::COOL);

    // TODO We really need unzoomed circles
    let radius = Distance::meters(50.0);
    // Optimization
    let circle = Circle::new(Pt2D::zero(), radius).to_polygon();
    let circle_outline = Circle::new(Pt2D::zero(), radius)
        .to_outline(Distance::meters(3.0))
        .unwrap();

    timer.start_iter("draw stops", counts.borrow().len());
    for (id, count) in counts.consume() {
        timer.next();
        let stop = &app.model.gtfs.stops[&id];

        let mut txt = describe::stop(stop);
        txt.add_line(Line(""));
        txt.add_line(format!("{} {} here", prettyprint_usize(count), label));

        let hitbox = circle.translate(stop.pos.x(), stop.pos.y());
        let mut batch = GeomBatch::new();

        let color = if max == 0.0 {
            scale.eval(0.0)
        } else {
            scale.eval(count as f64 / max)
        };
        batch.push(color, hitbox.clone());
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

    // Also update the panel here, since we have the counts
    let info = ColorLegend::gradient(
        ctx,
        &scale,
        vec!["0".to_string(), prettyprint_usize(max as usize)],
    );
    panel.replace(ctx, "stop style info", info);
}