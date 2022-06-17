mod events;

use abstutil::prettyprint_usize;
use chrono::Datelike;
use geom::{Circle, Distance, Duration, Pt2D, Speed, UnitFmt};
use widgetry::mapspace::{ObjectID, World};
use widgetry::{
    Cached, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, State, Text,
    TextExt, Toggle, UpdateType, Widget,
};

use model::gtfs::DateFilter;
use model::{Vehicle, VehicleID};

use self::events::Events;
use crate::components::{describe, MainMenu, TimeControls};
use crate::{App, Transition};

pub struct Replay {
    panel: Panel,
    time_controls: TimeControls,
    world: World<Obj>,
    hover_path: Cached<Obj, Drawable>,
    events: Events,
    prev_events: usize,
}

impl Replay {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = Self {
            panel: crate::components::MainMenu::panel(ctx, crate::components::Mode::Replay),
            time_controls: TimeControls::new(ctx, app),
            world: make_static_world(ctx, app),
            hover_path: Cached::new(),
            events: Events::ticketing(&app.model),
            prev_events: 0,
        };
        let controls = Widget::col(vec![
            format!(
                "Date: {} ({})",
                app.model.main_date,
                app.model.main_date.weekday()
            )
            .text_widget(ctx),
            // TODO The order of these always feels so backwards...
            Toggle::choice(ctx, "trajectory source", "BIL", "AVL", Key::T, false),
        ]);
        state.panel.replace(ctx, "contents", controls);

        state.on_time_change(ctx, app);
        Box::new(state)
    }

    fn on_time_change(&mut self, ctx: &mut EventCtx, app: &App) {
        let stats = update_world(
            ctx,
            app,
            &mut self.world,
            &self.events,
            &mut self.prev_events,
            self.panel.is_checked("trajectory source"),
        );
        self.time_controls.panel.replace(ctx, "stats", stats);
    }
}

impl State<App> for Replay {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        app.sync_mapbox(ctx);

        let prev_time = app.time;
        self.time_controls.event(ctx, app);
        if app.time != prev_time {
            self.on_time_change(ctx, app);
        }

        self.world.event(ctx);

        if !self.time_controls.is_paused() {
            ctx.request_update(UpdateType::Game);
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if let Some(t) = MainMenu::on_click(ctx, app, x.as_ref()) {
                    return t;
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed(_) => {
                // Trajectory source
                self.on_time_change(ctx, app);
                self.hover_path.clear();
            }
            _ => {}
        }

        self.hover_path
            .update(self.world.get_hovering(), |obj| match obj {
                Obj::Bus(id) => {
                    let vehicle = &app.model.vehicles[id.0];
                    let (main_trajectory, alt_trajectory) =
                        if self.panel.is_checked("trajectory source") {
                            (
                                vehicle.alt_trajectory.as_ref().unwrap(),
                                Some(&vehicle.trajectory),
                            )
                        } else {
                            (&vehicle.trajectory, vehicle.alt_trajectory.as_ref())
                        };

                    let mut batch = GeomBatch::new();
                    batch.push(
                        Color::CYAN,
                        main_trajectory
                            .as_polyline()
                            .make_polygons(Distance::meters(5.0)),
                    );

                    // Show where the vehicle is in the other trajectory right now
                    if let Some((pos2, _)) = alt_trajectory.and_then(|t| t.interpolate(app.time)) {
                        let (pos1, _) = main_trajectory.interpolate(app.time).unwrap();
                        if let Ok(line) = geom::Line::new(pos1, pos2) {
                            batch.push(Color::PINK, line.make_polygons(Distance::meters(5.0)));
                        }
                    }

                    ctx.upload(batch)
                }
                Obj::Stop(_) | Obj::Event(_) => Drawable::empty(ctx),
            });

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.time_controls.draw(g);
        self.world.draw(g);
        if let Some(draw) = self.hover_path.value() {
            g.redraw(draw);
        }
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    Bus(VehicleID),
    Stop(usize),
    Event(usize),
}
impl ObjectID for Obj {}

fn make_static_world(ctx: &mut EventCtx, app: &App) -> World<Obj> {
    let mut world = World::bounded(&app.model.bounds);

    // TODO We really need unzoomed circles
    let radius = Distance::meters(50.0);

    // Optimization
    let circle = Circle::new(Pt2D::zero(), radius).to_polygon();

    for (idx, stop) in app.model.gtfs.stops.values().enumerate() {
        world
            // TODO Need to assign numeric IDs in the model
            .add(Obj::Stop(idx))
            .hitbox(circle.translate(stop.pos.x(), stop.pos.y()))
            .draw_color(Color::BLUE)
            .hover_alpha(0.5)
            .tooltip(describe::stop(stop))
            .build(ctx);
    }

    world.initialize_hover(ctx);

    world
}

// Returns stats
fn update_world(
    ctx: &mut EventCtx,
    app: &App,
    world: &mut World<Obj>,
    events: &Events,
    prev_events: &mut usize,
    use_alt_trajectory_source: bool,
) -> Widget {
    // Delete all existing vehicles
    for vehicle in &app.model.vehicles {
        world.maybe_delete(Obj::Bus(vehicle.id));
    }

    // TODO We really need to be able to mix and match Worlds, or have a concept of layers.
    // Or... just pass a callback and say whether to retain objects or not.
    for ev in 0..*prev_events {
        world.delete(Obj::Event(ev));
    }
    *prev_events = 0;

    // TODO UnitFmt::metric()?
    let metric = UnitFmt {
        round_durations: false,
        metric: true,
    };
    let radius = Distance::meters(50.0);

    let mut away = 0;
    let mut idling = 0;
    let mut moving = 0;

    let get_vehicle_state = |vehicle: &Vehicle| {
        if use_alt_trajectory_source {
            if let Some(ref trajectory) = vehicle.alt_trajectory {
                trajectory.interpolate(app.time)
            } else {
                None
            }
        } else {
            vehicle.trajectory.interpolate(app.time)
        }
    };

    for vehicle in &app.model.vehicles {
        if let Some((pos, speed)) = get_vehicle_state(vehicle) {
            if speed == Speed::ZERO {
                idling += 1;
            } else {
                moving += 1;
            }

            world
                .add(Obj::Bus(vehicle.id))
                .hitbox(Circle::new(pos, radius).to_polygon())
                .draw_color(Color::RED)
                .hover_alpha(0.5)
                .tooltip(Text::from(format!(
                    "{:?} currently has speed {}",
                    vehicle.original_id,
                    speed.to_string(&metric)
                )))
                .build(ctx);
        } else {
            away += 1;
        }
    }

    let lookback = Duration::seconds(10.0);

    for ev in events.events_at(app.time, lookback) {
        let mut txt = Text::from(ev.describe());
        // 0 when the event occurs, then increases to 1
        let decay = (app.time - ev.time) / lookback;

        let mut hover = GeomBatch::new();
        hover.push(Color::GREEN, Circle::new(ev.pos, radius).to_polygon());
        // Where's the bus at this time?
        if let Some(vehicle) = app.model.lookup_vehicle(&ev.vehicle_name) {
            if let Some((pos, _)) = get_vehicle_state(vehicle) {
                if let Ok(line) = geom::Line::new(ev.pos, pos) {
                    hover.push(Color::YELLOW, line.make_polygons(Distance::meters(15.0)));
                    txt.add_line(format!(
                        "Bus is {} away from ticketing event",
                        line.length()
                    ));
                }
            }
        }
        // What routes match?
        let services = app
            .model
            .gtfs
            .calendar
            .services_matching_dates(&DateFilter::SingleDay(app.model.main_date));
        let mut matching_routes = 0;
        for route in app.model.gtfs.routes.values() {
            if route.short_name.as_ref() != Some(&ev.route_short_name) {
                continue;
            }
            for variant in &route.variants {
                if !services.contains(&variant.service_id) {
                    continue;
                }
                matching_routes += 1;
                if let Ok(pl) = variant.polyline(&app.model.gtfs) {
                    hover.push(Color::PURPLE, pl.make_polygons(Distance::meters(10.0)));
                }
            }
        }
        txt.add_line(format!("{matching_routes} route variants match"));

        world
            .add(Obj::Event(*prev_events))
            .hitbox(Circle::new(ev.pos, radius).to_polygon())
            .draw_color(Color::GREEN.alpha(1.0 - decay as f32))
            .draw_hovered(hover)
            .tooltip(txt)
            .build(ctx);
        *prev_events += 1;
    }

    world.initialize_hover(ctx);

    Text::from_multiline(vec![
        Line(format!("Away: {}", prettyprint_usize(away))),
        Line(format!("Idling: {}", prettyprint_usize(idling))),
        Line(format!("Moving: {}", prettyprint_usize(moving))),
        Line(format!(
            "Stops: {}",
            prettyprint_usize(app.model.gtfs.stops.len())
        )),
    ])
    .into_widget(ctx)
}
