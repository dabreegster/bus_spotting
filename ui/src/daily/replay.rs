use abstutil::prettyprint_usize;
use chrono::Datelike;
use geom::{Circle, Distance, Duration, Pt2D, Time, UnitFmt};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::tools::PromptInput;
use widgetry::{
    include_labeled_bytes, lctrl, Cached, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx,
    Key, Line, Outcome, Panel, State, Text, TextExt, UpdateType, Widget,
};

use gtfs::{DateFilter, RouteVariantID, StopID};
use model::VehicleID;

use super::events::Events;
use super::{App, TimeControls, Transition};
use crate::components::{describe, MainMenu};

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
            panel: crate::components::MainMenu::panel(ctx),
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
            format!("Date: {} ({})", app.model.date, app.model.date.weekday()).text_widget(ctx),
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

    pub fn use_hyperlink_state(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        hyperlink: (VehicleID, RouteVariantID, Time),
    ) {
        let (vehicle, variant, time) = hyperlink;
        self.on_select_vehicle(ctx, app, vehicle, Some(variant));
        self.on_select_variant(ctx, app, Some(variant));
        app.time = time;
        self.on_time_change(ctx, app);
        warp_to_vehicle_at_current_time(ctx, app, vehicle);
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

        if let Some(v) = self.selected_vehicle {
            // What're they doing right now?
            let label = if let Some(ev) = app.model.most_recent_boarding_event_for_bus(v, app.time)
            {
                Text::from_multiline(vec![
                    Line(format!("Currently serving: {:?}", ev.variant)),
                    Line(format!("(last stop {} ago)", app.time - ev.arrival_time)),
                ])
                .into_widget(ctx)
            } else {
                format!("Currently serving: ???").text_widget(ctx)
            };
            self.panel.replace(ctx, "current route", label);
        }
    }

    fn on_select_vehicle(
        &mut self,
        ctx: &mut EventCtx,
        app: &App,
        id: VehicleID,
        variant: Option<RouteVariantID>,
    ) {
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
            format!("Currently serving: ???")
                .text_widget(ctx)
                .named("current route"),
            Widget::row(vec![
                "Show stops for variant: ".text_widget(ctx),
                Widget::dropdown(ctx, "variant stops", variant, stops_choices),
            ]),
            ctx.style()
                .btn_outline
                .text("view schedule")
                .hotkey(Key::S)
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("compare trajectories (by variants)")
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("compare trajectories (by trips)")
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("score against trips")
                .build_def(ctx),
            if cfg!(not(target_arch = "wasm32")) {
                ctx.style()
                    .btn_outline
                    .text("write trajectory to CSV")
                    .build_def(ctx)
            } else {
                Widget::nothing()
            },
        ];
        for v in app.model.vehicle_to_possible_routes(id) {
            controls.push(
                ctx.style()
                    .btn_outline
                    .text(format!("match to variant {}", v.0))
                    .build_def(ctx),
            );
        }

        self.panel
            .replace(ctx, "vehicle controls", Widget::col(controls).section(ctx));
    }

    fn on_select_variant(
        &mut self,
        ctx: &mut EventCtx,
        app: &App,
        variant: Option<RouteVariantID>,
    ) {
        let mut batch = GeomBatch::new();
        if let Some(v) = variant {
            for (idx, id) in app.model.gtfs.variant(v).stops().into_iter().enumerate() {
                let pt = app.model.gtfs.stops[&id].pos;
                batch.append(
                    Text::from(Line(format!("{}", idx + 1)).fg(Color::WHITE))
                        .render(ctx)
                        .centered_on(pt),
                );
                if let Ok(p) =
                    Circle::new(pt, Distance::meters(50.0)).to_outline(Distance::meters(3.0))
                {
                    batch.push(Color::WHITE, p);
                }
            }
        }
        self.draw_stop_order = batch.upload(ctx);
    }
}

impl State<App> for Replay {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        app.maplibre.sync(ctx, &app.model.gps_bounds);

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
                self.on_select_vehicle(ctx, app, id, None);
                self.on_time_change(ctx, app);
            }
            WorldOutcome::ClickedObject(Obj::Stop(stop)) => {
                self.world.hack_unset_hovering();

                let stop = &app.model.gtfs.stops[&stop];
                let variants = stop
                    .route_variants
                    .iter()
                    .cloned()
                    .collect::<Vec<RouteVariantID>>();
                let first = variants[0];
                return Transition::Push(super::stop::StopInfo::new_state(
                    ctx, app, stop, variants, first,
                ));
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
                        warp_to_vehicle_at_current_time(ctx, app, self.selected_vehicle.unwrap());
                        return Transition::Keep;
                    }
                    "view schedule" => {
                        return view_schedule(ctx, app, self.selected_vehicle.unwrap());
                    }
                    "compare trajectories (by variants)" => {
                        let id = self.selected_vehicle.unwrap();
                        let vehicle = &app.model.vehicles[id.0];
                        let mut list = vec![("AVL".to_string(), vehicle.trajectory.clone())];
                        if let Ok(more) = app.model.possible_route_trajectories_for_vehicle(id) {
                            list.extend(more);
                        }
                        let clip_avl_time = false;
                        return Transition::Push(super::trajectories::Compare::new_state(
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
                        return Transition::Push(super::trajectories::Compare::new_state(
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
                    return Transition::Push(super::vehicle_route::Viewer::new_state(
                        ctx,
                        app,
                        self.selected_vehicle.unwrap(),
                        variant,
                    ));
                }

                if let Some(t) = MainMenu::on_click_daily(ctx, x.as_ref()) {
                    return t;
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed(x) => match x.as_ref() {
                "variant stops" => {
                    self.on_select_variant(ctx, app, self.panel.dropdown_value("variant stops"));
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
    let mut world = World::new();

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

    let mut serving_route = 0;
    let mut not_serving_route = 0;

    for vehicle in &app.model.vehicles {
        if let Some((pos, speed)) = vehicle.trajectory.interpolate(app.time) {
            let current_trip = vehicle.timetable.get_at_time(app.time);
            if current_trip.is_some() {
                serving_route += 1;
            } else {
                not_serving_route += 1;
            }

            let color = if Some(vehicle.id) == selected_vehicle {
                Color::YELLOW
            } else if current_trip.is_some() {
                Color::RED
            } else {
                Color::PINK
            };

            world
                .add(Obj::Bus(vehicle.id))
                // Use this for the vehicle radius, so it's visually clear if we're close enough to
                // a stop for it to count
                .hitbox(Circle::new(pos, model::BUS_TO_STOP_THRESHOLD).to_polygon())
                .draw_color(color)
                .hover_alpha(0.5)
                .tooltip(Text::from(format!(
                    "{:?} currently has speed {}, doing {:?}",
                    vehicle.original_id,
                    speed.to_string(&UnitFmt::metric()),
                    current_trip
                )))
                .clickable()
                .build(ctx);
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
            .services_matching_dates(&DateFilter::SingleDay(app.model.date));
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

    format!(
        "{} serving a route, {} not",
        prettyprint_usize(serving_route),
        prettyprint_usize(not_serving_route)
    )
    .text_widget(ctx)
}

fn warp_to_vehicle_at_current_time(ctx: &mut EventCtx, app: &App, vehicle: VehicleID) {
    if let Some((pt, _)) = app.model.vehicles[vehicle.0]
        .trajectory
        .interpolate(app.time)
    {
        ctx.canvas.cam_zoom = 1.0;
        ctx.canvas.center_on_map_pt(pt);
    }
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
                            state.on_select_vehicle(ctx, app, vehicle.id, None);
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

fn view_schedule(ctx: &mut EventCtx, app: &App, vehicle: VehicleID) -> Transition {
    let mut page = super::page::PageBuilder::new();
    let mut col = vec![
        "See STDOUT for skipped trips".text_widget(ctx),
        // TODO blank line
    ];

    let mut last_time = None;
    let debug = true;
    for trip in app.model.infer_vehicle_schedule(vehicle, debug, None) {
        if let Some(t) = last_time {
            col.push(format!("{} gap", trip.start_time() - t).text_widget(ctx));
        }
        last_time = Some(trip.end_time());
        col.push(page.btn_data(
            ctx,
            ctx.style().btn_plain.text(trip.summary()),
            (vehicle, trip.variant, trip.start_time()),
        ));
    }

    // TODO Up to this point, it's great! Next things I want:
    // - Specify routing for every "page" in the app, with some kind of enum. (It could map to a
    //   URL string if needed)
    // - Mostly ditch the state/transition concept. Universal routing to pages.
    // - (With history controls)
    // Different pages for base replayer, vs replayer with vehicle, vs replayer with vehicle and
    // variant selected?

    // TODO Actually, warp to vehicle + route schedule probably
    Transition::Push(page.build(
        ctx,
        "Vehicle schedule",
        Widget::col(col),
        Box::new(|_, _, hyperlink| {
            Transition::Multi(vec![
                Transition::Pop,
                Transition::ModifyState(Box::new(move |state, ctx, app| {
                    let state = state.downcast_mut::<Replay>().unwrap();
                    state.use_hyperlink_state(ctx, app, hyperlink);
                })),
            ])
        }),
    ))
}
