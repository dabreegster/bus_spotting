#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod components;
mod network;
mod replay;
mod stop;

use abstutil::Timer;
use anyhow::Result;
use geom::{Bounds, Duration, Time};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use widgetry::{Canvas, Color, EventCtx, GfxCtx, Settings, SharedAppState};

use model::{Model, VehicleID};

#[derive(StructOpt)]
struct Args {
    /// The path to a previously built and serialized model
    #[structopt(long)]
    model: Option<String>,
    /// The path to a .zip file with raw data to import
    #[structopt(long)]
    import_zip: Option<String>,
}

impl Args {
    // TODO These args only make sense on native, because they read files
    fn load(mut self, timer: &mut Timer) -> Result<Model> {
        if let Some(path) = self.model.take() {
            if self.import_zip.is_some() {
                bail!("You can't specify both --model and --import-zip");
            }

            let bytes = fs_err::read(path)?;
            let decoded = base64::decode(bytes)?;
            return abstutil::from_binary::<Model>(&decoded);
        }
        if self.import_zip.is_none() {
            return Ok(Model::empty());
        }
        let bytes = fs_err::read(self.import_zip.take().unwrap())?;
        let model = Model::import_zip_bytes(bytes, timer)?;

        let save_model = base64::encode(abstutil::to_binary(&model));
        abstio::write_file("data/output/model.bin".to_string(), save_model)?;
        Ok(model)
    }
}

fn run(settings: Settings) {
    abstutil::logger::setup();

    let args = Args::from_iter(abstutil::cli_args());

    widgetry::run(settings, move |ctx| {
        let model = ctx.loading_screen("initialize model", |_, timer| {
            let mut model = args.load(timer).unwrap();
            // TODO tmp
            //model.segment(timer).unwrap();
            model.set_alt_trajectories_from_ticketing();
            model
        });

        let mut app = App::new(ctx, model);

        let mut states = vec![crate::network::Viewer::new_state(ctx, &app)];

        // This only makes sense on native, with the same model used across different runs.
        // before_quit is never called on web, and web starts with an empty model.
        if let Ok(savestate) = abstio::maybe_read_json::<Savestate>(
            "data/save.json".to_string(),
            &mut Timer::throwaway(),
        ) {
            ctx.canvas.cam_x = savestate.cam_x;
            ctx.canvas.cam_y = savestate.cam_y;
            ctx.canvas.cam_zoom = savestate.cam_zoom;
            if let SavestateMode::Replayer(time, _selected_vehicle) = savestate.mode {
                app.time = time;
                // TODO Also restore selected_vehicle
                states = vec![crate::replay::Replay::new_state(ctx, &app)];
            }
        }

        (app, states)
    });
}

pub fn main() {
    let settings = Settings::new("Bus Spotting");
    run(settings);
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run_wasm() {
    run(Settings::new("Bus Spotting").root_dom_element_id("loading".to_string()));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn sync_mapbox_canvas(lon1: f64, lat1: f64, lon2: f64, lat2: f64);
}

pub struct App {
    model: Model,

    // Mostly applies to the network viewer now, but we want to preserve it when switching modes
    filters: network::Filters,

    // Sticky state for the replayer
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
        abstio::write_json("data/save.json".to_string(), &ss);
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

            filters: network::Filters::new(),

            time: Time::START_OF_DAY,
            time_increment: Duration::minutes(10),

            mapbox_bounds: Bounds::new(),

            savestate_mode: SavestateMode::NetworkViewer,
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
}

#[derive(Serialize, Deserialize)]
pub struct Savestate {
    cam_x: f64,
    cam_y: f64,
    cam_zoom: f64,
    mode: SavestateMode,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SavestateMode {
    NetworkViewer,
    Replayer(Time, Option<VehicleID>),
}
