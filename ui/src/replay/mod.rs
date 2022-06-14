mod events;

use abstutil::prettyprint_usize;
use geom::{Circle, Distance, Duration, Pt2D, Speed, UnitFmt};
use widgetry::mapspace::{ObjectID, World};
use widgetry::{
    Cached, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, State, Text,
    TextExt, UpdateType, Widget,
};

use model::VehicleID;

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
        let label = format!("Date: {}", app.model.main_date).text_widget(ctx);
        state.panel.replace(ctx, "contents", label);

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
        );
        self.time_controls.panel.replace(ctx, "stats", stats);
    }
}

impl State<App> for Replay {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let prev_time = app.time;
        self.time_controls.event(ctx, app);
        if app.time != prev_time {
            self.on_time_change(ctx, app);
        }

        self.world.event(ctx);

        self.hover_path
            .update(self.world.get_hovering(), |obj| match obj {
                Obj::Bus(id) => {
                    let mut batch = GeomBatch::new();
                    batch.push(
                        Color::CYAN,
                        app.model.vehicles[id.0]
                            .trajectory
                            .as_polyline()
                            .make_polygons(Distance::meters(5.0)),
                    );
                    ctx.upload(batch)
                }
                Obj::Stop(_) | Obj::Event(_) => Drawable::empty(ctx),
            });

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
            _ => {}
        }

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

    // Show the bounds of the world
    world.draw_master_batch(
        ctx,
        GeomBatch::from(vec![(Color::grey(0.1), app.model.bounds.get_rectangle())]),
    );

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

    for vehicle in &app.model.vehicles {
        if let Some((pos, speed)) = vehicle.trajectory.interpolate(app.time) {
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
        let mut matching_routes = 0;
        for route in app.model.gtfs.routes.values() {
            if route.short_name.as_ref() != Some(&ev.route_short_name) {
                continue;
            }
            for variant in &route.variants {
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
