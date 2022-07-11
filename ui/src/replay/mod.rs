mod events;
mod trajectories;
mod vehicle_route;

use abstutil::prettyprint_usize;
use chrono::Datelike;
use geom::{Circle, Distance, Duration, Pt2D, Speed, Time, UnitFmt};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::tools::{PopupMsg, PromptInput};
use widgetry::{
    include_labeled_bytes, lctrl, Cached, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx,
    Key, Line, Outcome, Panel, State, Text, TextExt, UpdateType, Widget,
};

use model::gtfs::{DateFilter, RouteVariantID, StopID};
use model::VehicleID;

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
    snap_to_trajectory: Cached<Pt2D, (Text, Drawable, Option<Time>)>,
    draw_stop_order: Drawable,
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
            snap_to_trajectory: Cached::new(),
            draw_stop_order: Drawable::empty(ctx),
        };
        let controls = Widget::col(vec![
            format!(
                "Date: {} ({})",
                app.model.main_date,
                app.model.main_date.weekday()
            )
            .text_widget(ctx),
            ctx.style()
                .btn_outline
                .text("Replace vehicles with GTFS")
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Warp to vehicle")
                .hotkey(lctrl(Key::J))
                .build_def(ctx),
            format!("No vehicle selected")
                .text_widget(ctx)
                .named("vehicle controls"),
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
            self.selected_vehicle,
        );
        self.time_controls.panel.replace(ctx, "stats", stats);
    }

    fn on_select_vehicle(&mut self, ctx: &mut EventCtx, app: &App, id: VehicleID) {
        self.selected_vehicle = Some(id);
        self.draw_stop_order = Drawable::empty(ctx);

        let mut stops_choices = vec![Choice::<Option<RouteVariantID>>::new("---", None)];
        for v in app.model.vehicle_to_possible_routes(id) {
            stops_choices.push(Choice::new(format!("{:?}", v), Some(v)));
        }

        let mut controls = vec![
            Widget::row(vec![
                format!("Selected {:?}", id).text_widget(ctx),
                ctx.style()
                    .btn_plain
                    .icon_bytes(include_labeled_bytes!("../../assets/location.svg"))
                    .build_widget(ctx, "goto this vehicle"),
            ]),
            Widget::row(vec![
                "Show stops for variant: ".text_widget(ctx),
                Widget::dropdown(ctx, "variant stops", None, stops_choices),
            ]),
            ctx.style()
                .btn_plain
                .text("view schedule")
                .hotkey(Key::S)
                .build_def(ctx),
            ctx.style()
                .btn_plain
                .text("compare trajectories (by variants)")
                .build_def(ctx),
            ctx.style()
                .btn_plain
                .text("compare trajectories (by trips)")
                .build_def(ctx),
            ctx.style()
                .btn_plain
                .text("score against trips")
                .build_def(ctx),
            if cfg!(not(target_arch = "wasm32")) {
                ctx.style()
                    .btn_plain
                    .text("write trajectory to CSV")
                    .build_def(ctx)
            } else {
                Widget::nothing()
            },
        ];
        for v in app.model.vehicle_to_possible_routes(id) {
            controls.push(
                ctx.style()
                    .btn_plain
                    .text(format!("match to variant {}", v.0))
                    .build_def(ctx),
            );
        }

        self.panel
            .replace(ctx, "vehicle controls", Widget::col(controls).section(ctx));
    }
}

impl State<App> for Replay {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        app.savestate_mode = crate::SavestateMode::Replayer(app.time, self.selected_vehicle);
        app.sync_mapbox(ctx);

        let prev_time = app.time;
        if let Some(t) = self.time_controls.event(ctx, app) {
            return t;
        }
        if app.time != prev_time {
            self.on_time_change(ctx, app);
        }

        match self.world.event(ctx) {
            WorldOutcome::ClickedFreeSpace(_) => {
                self.selected_vehicle = None;
                let label = format!("No vehicle selected").text_widget(ctx);
                self.panel.replace(ctx, "vehicle controls", label);
                self.draw_stop_order = Drawable::empty(ctx);
                self.on_time_change(ctx, app);
            }
            WorldOutcome::ClickedObject(Obj::Bus(id)) => {
                self.on_select_vehicle(ctx, app, id);
                self.on_time_change(ctx, app);
            }
            WorldOutcome::ClickedObject(Obj::Stop(stop)) => {
                if let Some(vehicle) = self.selected_vehicle {
                    let stop_pos = app.model.gtfs.stops[&stop].pos;
                    let threshold = Distance::meters(10.0);
                    return Transition::Push(PopupMsg::new_state(
                        ctx,
                        "Vehicle near this stop at...",
                        app.model.vehicles[vehicle.0]
                            .trajectory
                            .times_near_pos(stop_pos, threshold)
                            .into_iter()
                            .map(|(t, _)| t.to_string())
                            .collect(),
                    ));
                }
            }
            _ => {}
        }

        if !self.time_controls.is_paused() {
            ctx.request_update(UpdateType::Game);
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                match x.as_ref() {
                    "Replace vehicles with GTFS" => {
                        app.model.replace_vehicles_with_gtfs();
                        return Transition::Replace(Self::new_state(ctx, app));
                    }
                    "Warp to vehicle" => {
                        return warp_to_vehicle(ctx);
                    }
                    "goto this vehicle" => {
                        if let Some((pt, _)) = app.model.vehicles[self.selected_vehicle.unwrap().0]
                            .trajectory
                            .interpolate(app.time)
                        {
                            ctx.canvas.cam_zoom = 1.0;
                            ctx.canvas.center_on_map_pt(pt);
                        }
                        return Transition::Keep;
                    }
                    "view schedule" => {
                        let mut lines =
                            vec!["See STDOUT for skipped trips".to_string(), String::new()];
                        let mut last_time = None;
                        let debug = true;
                        for trip in app
                            .model
                            .infer_vehicle_schedule(self.selected_vehicle.unwrap(), debug)
                        {
                            if let Some(t) = last_time {
                                lines.push(format!("{} gap", trip.start_time() - t));
                            }
                            last_time = Some(trip.end_time());
                            lines.push(trip.summary());
                        }
                        return Transition::Push(PopupMsg::new_state(
                            ctx,
                            "Vehicle schedule",
                            lines,
                        ));
                    }
                    "compare trajectories (by variants)" => {
                        let id = self.selected_vehicle.unwrap();
                        let vehicle = &app.model.vehicles[id.0];
                        let mut list = vec![("AVL".to_string(), vehicle.trajectory.clone())];
                        if let Ok(more) = app.model.possible_route_trajectories_for_vehicle(id) {
                            list.extend(more);
                        }
                        let clip_avl_time = false;
                        return Transition::Push(trajectories::Compare::new_state(
                            ctx,
                            list,
                            clip_avl_time,
                        ));
                    }
                    "compare trajectories (by trips)" => {
                        let id = self.selected_vehicle.unwrap();
                        let vehicle = &app.model.vehicles[id.0];
                        let mut list = vec![("AVL".to_string(), vehicle.trajectory.clone())];
                        if let Ok(more) = app.model.possible_trip_trajectories_for_vehicle(id) {
                            list.extend(more);
                        }
                        let clip_avl_time = true;
                        return Transition::Push(trajectories::Compare::new_state(
                            ctx,
                            list,
                            clip_avl_time,
                        ));
                    }
                    "score against trips" => {
                        let id = self.selected_vehicle.unwrap();
                        println!("Matching {:?} to possible trips", id);
                        for (trip, score) in app
                            .model
                            .score_vehicle_similarity_to_trips(id)
                            .into_iter()
                            .take(5)
                        {
                            println!("- {:?} has score of {}", trip, score);
                        }
                        return Transition::Keep;
                    }
                    "write trajectory to CSV" => {
                        let vehicle = &app.model.vehicles[self.selected_vehicle.unwrap().0];
                        vehicle
                            .trajectory
                            .write_to_csv(
                                format!("trajectory_vehicle_{}.csv", vehicle.id.0),
                                &app.model.gps_bounds,
                            )
                            .unwrap();
                        return Transition::Keep;
                    }
                    _ => {}
                }

                if let Some(x) = x.strip_prefix("match to variant ") {
                    let variant = RouteVariantID(x.parse::<usize>().unwrap());
                    return Transition::Push(vehicle_route::Viewer::new_state(
                        ctx,
                        app,
                        self.selected_vehicle.unwrap(),
                        variant,
                    ));
                }

                if let Some(t) = MainMenu::on_click(ctx, app, x.as_ref()) {
                    return t;
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed(x) => match x.as_ref() {
                "variant stops" => {
                    let mut batch = GeomBatch::new();
                    if let Some(v) = self.panel.dropdown_value("variant stops") {
                        for (idx, id) in app.model.gtfs.variant(v).stops().into_iter().enumerate() {
                            let pt = app.model.gtfs.stops[&id].pos;
                            batch.append(
                                Text::from(Line(format!("{}", idx + 1)).fg(Color::WHITE))
                                    .render(ctx)
                                    .centered_on(pt),
                            );
                            if let Ok(p) = Circle::new(pt, Distance::meters(50.0))
                                .to_outline(Distance::meters(3.0))
                            {
                                batch.push(Color::WHITE, p);
                            }
                        }
                    }
                    self.draw_stop_order = batch.upload(ctx);
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        let focus = self
            .selected_vehicle
            .or_else(|| match self.world.get_hovering() {
                Some(Obj::Bus(id)) => Some(id),
                _ => None,
            });
        self.show_path.update(focus, |id| {
            let vehicle = &app.model.vehicles[id.0];
            let mut batch = GeomBatch::new();
            batch.push(
                Color::CYAN,
                vehicle
                    .trajectory
                    .as_polyline()
                    .make_polygons(Distance::meters(5.0)),
            );
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
                        let hits = vehicle
                            .trajectory
                            .times_near_pos(pt, Distance::meters(30.0));
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
        if let Some((txt, draw, _)) = self.snap_to_trajectory.value() {
            g.redraw(draw);
            g.draw_mouse_tooltip(txt.clone());
        }
        g.redraw(&self.draw_stop_order);
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
            .clickable()
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

    for vehicle in &app.model.vehicles {
        if let Some((pos, speed)) = vehicle.trajectory.interpolate(app.time) {
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
                // Use this for the vehicle radius, so it's visually clear if we're close enough to
                // a stop for it to count
                .hitbox(Circle::new(pos, model::BUS_TO_STOP_THRESHOLD).to_polygon())
                .draw_color(color)
                .hover_alpha(0.5)
                .tooltip(Text::from(format!(
                    "{:?} currently has speed {}",
                    vehicle.original_id,
                    speed.to_string(&UnitFmt::metric())
                )))
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
            if let Some((pos, _)) = vehicle.trajectory.interpolate(app.time) {
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

fn warp_to_vehicle(ctx: &mut EventCtx) -> Transition {
    Transition::Push(PromptInput::new_state(
        ctx,
        "Warp to what vehicle ID?",
        String::new(),
        Box::new(move |response, _, _| {
            Transition::Multi(vec![
                Transition::Pop,
                Transition::ModifyState(Box::new(move |state, ctx, app| {
                    let state = state.downcast_mut::<Replay>().unwrap();

                    if let Ok(x) = response.parse::<usize>() {
                        if let Some(vehicle) = app.model.vehicles.get(x) {
                            state.on_select_vehicle(ctx, app, vehicle.id);
                            // Redraw the selected vehicle
                            state.on_time_change(ctx, app);
                            ctx.canvas.cam_zoom = 1.0;
                            if let Some((pt, _)) = vehicle.trajectory.interpolate(app.time) {
                                ctx.canvas.center_on_map_pt(pt);
                            } else {
                                ctx.canvas
                                    .center_on_map_pt(vehicle.trajectory.as_polyline().first_pt());
                                app.time = vehicle.trajectory.start_time();
                                state.on_time_change(ctx, app);
                                // Immediately update this
                                state.time_controls.event(ctx, app);
                            }
                        }
                    }
                })),
            ])
        }),
    ))
}
