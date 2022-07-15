mod events;
mod page;
mod replay;
mod speed;
mod stop;
mod trajectories;
mod variant;
mod vehicle_route;

use geom::{Bounds, Duration, Time};
use serde::{Deserialize, Serialize};
use widgetry::{Canvas, Color, EventCtx, GfxCtx, SharedAppState};

use model::{Model, VehicleID};

pub use replay::Replay;
use speed::TimeControls;

pub struct App {
    model: Model,

    time: Time,
    time_increment: Duration,

    // Avoid syncing when bounds match
    #[allow(unused)]
    mapbox_bounds: Bounds,

    savestate_mode: SavestateMode,
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
            mode: self.savestate_mode.clone(),
        };
        abstio::write_json("data/save_replay.json".to_string(), &ss);
    }
}

pub type Transition = widgetry::Transition<App>;

impl App {
    pub fn new(ctx: &mut EventCtx, model: Model) -> Self {
        let bounds = &model.bounds;
        ctx.canvas.map_dims = (bounds.max_x, bounds.max_y);
        ctx.canvas.center_on_map_pt(bounds.center());

        Self {
            model,

            time: Time::START_OF_DAY,
            time_increment: Duration::minutes(10),

            mapbox_bounds: Bounds::new(),

            savestate_mode: SavestateMode::Replayer(Time::START_OF_DAY, None),
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
    pub fn restore_savestate(&mut self, ctx: &mut EventCtx) {
        if let Ok(savestate) = abstio::maybe_read_json::<Savestate>(
            "data/save_replay.json".to_string(),
            &mut abstutil::Timer::throwaway(),
        ) {
            ctx.canvas.cam_x = savestate.cam_x;
            ctx.canvas.cam_y = savestate.cam_y;
            ctx.canvas.cam_zoom = savestate.cam_zoom;
            if let SavestateMode::Replayer(time, _selected_vehicle) = savestate.mode {
                self.time = time;
                // TODO Also restore selected_vehicle
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Savestate {
    cam_x: f64,
    cam_y: f64,
    cam_zoom: f64,
    // TODO Embed?
    mode: SavestateMode,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SavestateMode {
    Replayer(Time, Option<VehicleID>),
}

fn compare_time(scheduled: Time, actual: Time) -> String {
    if scheduled == actual {
        return "on time".to_string();
    }
    if scheduled < actual {
        return format!("{} late", actual - scheduled);
    }
    format!("{} early", scheduled - actual)
}
