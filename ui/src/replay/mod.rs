mod events;

use abstutil::prettyprint_usize;
use chrono::Datelike;
use geom::{Circle, Distance, Duration, Pt2D, Speed, Time, UnitFmt};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Cached, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, State, Text,
    TextExt, Toggle, UpdateType, Widget,
};

use model::gtfs::{DateFilter, StopID};
use model::{Vehicle, VehicleID};

use self::events::Events;
use crate::components::{describe, MainMenu, TimeControls};
use crate::{App, Transition};

pub struct Replay {
    panel: Panel,
    time_controls: TimeControls,
    world: World<Obj>,
    events: Events,
    prev_events: usize,

    selected_vehicle: Option<VehicleID>,
    show_path: Cached<VehicleID, Drawable>,
    show_alt_position: Cached<VehicleID, Drawable>,
    snap_to_trajectory: Cached<Pt2D, (Text, Drawable, Option<Time>)>,
}

impl Replay {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut state = Self {
            panel: crate::components::MainMenu::panel(ctx, crate::components::Mode::Replay),
            time_controls: TimeControls::new(ctx, app),
            world: make_static_world(ctx, app),
            events: Events::ticketing(&app.model),
            prev_events: 0,

            selected_vehicle: None,
            show_path: Cached::new(),
            show_alt_position: Cached::new(),
            snap_to_trajectory: Cached::new(),
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
            format!("No vehicle selected")
                .text_widget(ctx)
                .named("current vehicle"),
            ctx.style()
                .btn_plain
                .text("Replace vehicles with GTFS")
                .build_def(ctx),
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
            self.selected_vehicle,
        );
        self.time_controls.panel.replace(ctx, "stats", stats);
        self.show_alt_position.clear();
    }
}

impl State<App> for Replay {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        app.savestate_mode = crate::SavestateMode::Replayer(app.time, self.selected_vehicle);
        app.sync_mapbox(ctx);

        let prev_time = app.time;
        self.time_controls.event(ctx, app);
        if app.time != prev_time {
            self.on_time_change(ctx, app);
        }

        match self.world.event(ctx) {
            WorldOutcome::ClickedFreeSpace(_) => {
                self.selected_vehicle = None;
                let label = format!("No vehicle selected").text_widget(ctx);
                self.panel.replace(ctx, "current vehicle", label);
                self.on_time_change(ctx, app);
            }
            WorldOutcome::ClickedObject(Obj::Bus(id)) => {
                self.selected_vehicle = Some(id);
                let label = format!("Selected {:?}", id).text_widget(ctx);
                self.panel.replace(ctx, "current vehicle", label);
                self.on_time_change(ctx, app);
            }
            WorldOutcome::Keypress("compare trajectories", Obj::Bus(id)) => {
                let vehicle = &app.model.vehicles[id.0];
                let mut list = vec![("AVL".to_string(), vehicle.trajectory.clone())];
                if let Some(ref t) = vehicle.alt_trajectory {
                    list.push(("BIL".to_string(), t.clone()));
                }
                if let Ok(more) = app.model.possible_trajectories_for_vehicle(id) {
                    list.extend(more);
                }
                return Transition::Push(crate::trajectories::Compare::new_state(ctx, list));
            }
            WorldOutcome::Keypress("score against trips", Obj::Bus(id)) => {
                println!("Matching {:?} to possible trips", id);
                for (trip, score) in app
                    .model
                    .score_vehicle_similarity_to_trips(id)
                    .into_iter()
                    .take(5)
                {
                    println!("- {:?} has score of {}", trip, score);
                }
            }
            WorldOutcome::Keypress("match to route shape", Obj::Bus(id)) => {
                app.model.match_to_route_shapes(id).unwrap();
            }
            _ => {}
        }

        if !self.time_controls.is_paused() {
            ctx.request_update(UpdateType::Game);
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if x == "Replace vehicles with GTFS" {
                    app.model.replace_vehicles_with_gtfs();
                    return Transition::Replace(Self::new_state(ctx, app));
                }

                if let Some(t) = MainMenu::on_click(ctx, app, x.as_ref()) {
                    return t;
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed(_) => {
                // Trajectory source
                self.on_time_change(ctx, app);
                self.show_path.clear();
                self.show_alt_position.clear();
            }
            _ => {}
        }

        let focus = self
            .selected_vehicle
            .or_else(|| match self.world.get_hovering() {
                Some(Obj::Bus(id)) => Some(id),
                _ => None,
            });
        // Note either trajectory may be unavailable at this time
        self.show_path.update(focus, |id| {
            let vehicle = &app.model.vehicles[id.0];
            let mut batch = GeomBatch::new();
            if let Some(trajectory) = if self.panel.is_checked("trajectory source") {
                vehicle.alt_trajectory.as_ref()
            } else {
                Some(&vehicle.trajectory)
            } {
                batch.push(
                    Color::CYAN,
                    trajectory
                        .as_polyline()
                        .make_polygons(Distance::meters(5.0)),
                );
            }

            ctx.upload(batch)
        });
        self.show_alt_position.update(focus, |id| {
            let vehicle = &app.model.vehicles[id.0];
            let (main_trajectory, alt_trajectory) = if self.panel.is_checked("trajectory source") {
                (vehicle.alt_trajectory.as_ref(), Some(&vehicle.trajectory))
            } else {
                (Some(&vehicle.trajectory), vehicle.alt_trajectory.as_ref())
            };

            let mut batch = GeomBatch::new();
            if let Some((pos2, _)) = alt_trajectory.and_then(|t| t.interpolate(app.time)) {
                if let Some((pos1, _)) = main_trajectory.and_then(|t| t.interpolate(app.time)) {
                    if let Ok(line) = geom::Line::new(pos1, pos2) {
                        batch.push(Color::PINK, line.make_polygons(Distance::meters(5.0)));
                    }
                }
            }

            ctx.upload(batch)
        });

        self.snap_to_trajectory
            .update(ctx.canvas.get_cursor_in_map_space(), |pt| {
                let mut txt = Text::new();
                let mut batch = GeomBatch::new();
                let mut time_warp = None;

                if let Some(id) = self.selected_vehicle {
                    if self.world.get_hovering().is_none() {
                        let vehicle = &app.model.vehicles[id.0];
                        if let Some(trajectory) = if self.panel.is_checked("trajectory source") {
                            vehicle.alt_trajectory.as_ref()
                        } else {
                            Some(&vehicle.trajectory)
                        } {
                            let hits = trajectory.times_near_pos(pt, Distance::meters(30.0));
                            if !hits.is_empty() {
                                batch.push(
                                    Color::CYAN,
                                    Circle::new(hits[0].1, Distance::meters(30.0)).to_polygon(),
                                );
                                let n = hits.len();
                                for (idx, (time, _)) in hits.into_iter().enumerate() {
                                    txt.add_line(Line(format!("Here at {time}")));
                                    if idx == 0 {
                                        time_warp = Some(time);
                                    }
                                    txt.append(Line("  (press W to time-warp)"));
                                    if idx == 4 {
                                        txt.append(Line(format!(" (and {} more times)", n - 5)));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                (txt, ctx.upload(batch), time_warp)
            });

        if let Some((_, _, Some(time))) = self.snap_to_trajectory.value() {
            if ctx.input.pressed(Key::W) {
                app.time = *time;
                self.on_time_change(ctx, app);
                // Immediately update this
                self.time_controls.event(ctx, app);
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.time_controls.draw(g);
        if let Some(draw) = self.show_path.value() {
            g.redraw(draw);
        }
        self.world.draw(g);
        if let Some(draw) = self.show_alt_position.value() {
            g.redraw(draw);
        }
        if let Some((txt, draw, _)) = self.snap_to_trajectory.value() {
            g.redraw(draw);
            g.draw_mouse_tooltip(txt.clone());
        }
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Obj {
    Bus(VehicleID),
    Stop(StopID),
    Event(usize),
}
impl ObjectID for Obj {}

fn make_static_world(ctx: &mut EventCtx, app: &App) -> World<Obj> {
    let mut world = World::bounded(&app.model.bounds);

    // TODO We really need unzoomed circles
    let radius = Distance::meters(50.0);

    // Optimization
    let circle = Circle::new(Pt2D::zero(), radius).to_polygon();

    for stop in app.model.gtfs.stops.values() {
        world
            .add(Obj::Stop(stop.id))
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
    selected_vehicle: Option<VehicleID>,
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

            let color = if Some(vehicle.id) == selected_vehicle {
                Color::YELLOW
            } else {
                Color::RED
            };

            world
                .add(Obj::Bus(vehicle.id))
                .hitbox(Circle::new(pos, radius).to_polygon())
                .draw_color(color)
                .hover_alpha(0.5)
                .tooltip(Text::from(format!(
                    "{:?} currently has speed {}",
                    vehicle.original_id,
                    speed.to_string(&UnitFmt::metric())
                )))
                .hotkey(Key::C, "compare trajectories")
                .hotkey(Key::S, "score against trips")
                .hotkey(Key::R, "match to route shape")
                .clickable()
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
