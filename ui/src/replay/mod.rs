mod events;
mod vehicle_route;

use abstutil::prettyprint_usize;
use chrono::Datelike;
use geom::{Circle, Distance, Duration, Pt2D, Speed, Time, UnitFmt};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::tools::{ChooseSomething, PopupMsg, PromptInput};
use widgetry::{
    include_labeled_bytes, lctrl, Cached, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx,
    Key, Line, Outcome, Panel, State, Text, TextExt, Toggle, UpdateType, Widget,
};

use model::gtfs::{DateFilter, RouteVariantID, StopID};
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
            show_alt_position: Cached::new(),
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
            // TODO The order of these always feels so backwards...
            Toggle::choice(ctx, "trajectory source", "BIL", "AVL", Key::T, false),
            Widget::row(vec![
                format!("No vehicle selected")
                    .text_widget(ctx)
                    .named("debug vehicle"),
                ctx.style()
                    .btn_plain
                    .icon_bytes(include_labeled_bytes!("../../assets/location.svg"))
                    .build_widget(ctx, "goto this vehicle"),
            ]),
            Widget::row(vec![
                "Show stops for variant: ".text_widget(ctx),
                Widget::dropdown(
                    ctx,
                    "variant stops",
                    None,
                    vec![Choice::<Option<RouteVariantID>>::new("---", None)],
                ),
            ]),
            ctx.style()
                .btn_outline
                .text("Replace vehicles with GTFS")
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Warp to vehicle")
                .hotkey(lctrl(Key::J))
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

    fn on_select_vehicle(&mut self, ctx: &mut EventCtx, app: &App, id: VehicleID) {
        self.selected_vehicle = Some(id);
        let btn = ctx
            .style()
            .btn_outline
            .text(format!("Selected {:?}", id))
            .hotkey(Key::D)
            .build_widget(ctx, "debug vehicle");
        self.panel.replace(ctx, "debug vehicle", btn);

        let mut choices = vec![Choice::<Option<RouteVariantID>>::new("---", None)];
        for v in app.model.vehicle_to_possible_routes(id) {
            choices.push(Choice::new(format!("{:?}", v), Some(v)));
        }
        let dropdown = Widget::dropdown(ctx, "variant stops", None, choices);
        self.panel.replace(ctx, "variant stops", dropdown);
        self.draw_stop_order = Drawable::empty(ctx);
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
                self.panel.replace(ctx, "debug vehicle", label);
                let dropdown = Widget::dropdown(
                    ctx,
                    "variant stops",
                    None,
                    vec![Choice::<Option<RouteVariantID>>::new("---", None)],
                );
                self.panel.replace(ctx, "variant stops", dropdown);
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
                    "debug vehicle" => {
                        return open_vehicle_menu(ctx, app, self.selected_vehicle.unwrap());
                    }
                    "Warp to vehicle" => {
                        return warp_to_vehicle(ctx);
                    }
                    "goto this vehicle" => {
                        if let Some(id) = self.selected_vehicle {
                            if let Some((pt, _)) =
                                app.model.vehicles[id.0].trajectory.interpolate(app.time)
                            {
                                ctx.canvas.cam_zoom = 1.0;
                                ctx.canvas.center_on_map_pt(pt);
                            }
                        }
                        return Transition::Keep;
                    }
                    _ => {}
                }

                if let Some(t) = MainMenu::on_click(ctx, app, x.as_ref()) {
                    return t;
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed(x) => match x.as_ref() {
                "AVL" | "BIL" => {
                    self.on_time_change(ctx, app);
                    self.show_path.clear();
                    self.show_alt_position.clear();
                }
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

fn open_vehicle_menu(ctx: &mut EventCtx, app: &App, id: VehicleID) -> Transition {
    let mut choices = vec![
        Choice::string("compare trajectories (by variants)"),
        Choice::string("compare trajectories (by trips)"),
        Choice::string("score against trips"),
    ];

    for v in app.model.vehicle_to_possible_routes(id) {
        choices.push(Choice::string(&format!("match to variant {}", v.0)));
    }

    Transition::Push(ChooseSomething::new_state(
        ctx,
        "Debug this vehicle",
        choices,
        Box::new(move |choice, ctx, app| match choice.as_ref() {
            "compare trajectories (by variants)" => {
                let vehicle = &app.model.vehicles[id.0];
                let mut list = vec![("AVL".to_string(), vehicle.trajectory.clone())];
                if let Some(ref t) = vehicle.alt_trajectory {
                    list.push(("BIL".to_string(), t.clone()));
                }
                if let Ok(more) = app.model.possible_route_trajectories_for_vehicle(id) {
                    list.extend(more);
                }
                let clip_avl_time = false;
                Transition::Replace(crate::trajectories::Compare::new_state(
                    ctx,
                    list,
                    clip_avl_time,
                ))
            }
            "compare trajectories (by trips)" => {
                let vehicle = &app.model.vehicles[id.0];
                let mut list = vec![("AVL".to_string(), vehicle.trajectory.clone())];
                if let Ok(more) = app.model.possible_trip_trajectories_for_vehicle(id) {
                    list.extend(more);
                }
                let clip_avl_time = true;
                Transition::Replace(crate::trajectories::Compare::new_state(
                    ctx,
                    list,
                    clip_avl_time,
                ))
            }
            "score against trips" => {
                println!("Matching {:?} to possible trips", id);
                for (trip, score) in app
                    .model
                    .score_vehicle_similarity_to_trips(id)
                    .into_iter()
                    .take(5)
                {
                    println!("- {:?} has score of {}", trip, score);
                }
                Transition::Pop
            }
            x => {
                if let Some(x) = x.strip_prefix("match to variant ") {
                    let vehicle = &app.model.vehicles[id.0];
                    let variant = RouteVariantID(x.parse::<usize>().unwrap());
                    return Transition::Replace(vehicle_route::Viewer::new_state(
                        ctx,
                        app,
                        vehicle.trajectory.clone(),
                        variant,
                    ));
                }
                unreachable!();
            }
        }),
    ))
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
