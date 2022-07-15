#[macro_use]
extern crate log;

mod components;
mod network;
mod replay;

use geom::{Bounds, Duration, Time};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use widgetry::{Canvas, Color, EventCtx, GfxCtx, Settings, SharedAppState};

use model::{Model, MultidayModel, VehicleID};

// TODO These args only make sense on native, because they read files
// TODO Could probably make this an optional enum now
#[derive(StructOpt)]
struct Args {
    /// The path to a previously built and serialized daily model
    #[structopt(long)]
    daily: Option<String>,
    /// The path to a previously built and serialized multiday model
    #[structopt(long)]
    multiday: Option<String>,
    /// The path to a .zip file with raw data to import. This'll enter the multiday mode after
    /// importing
    #[structopt(long)]
    import_zip: Option<String>,
}

fn run(settings: Settings) {
    abstutil::logger::setup();

    let args = Args::from_iter(abstutil::cli_args());
    let n = [&args.daily, &args.multiday, &args.import_zip]
        .iter()
        .filter(|x| x.is_some())
        .count();
    if n == 0 {
        // Empty network view
        widgetry::run(settings, |ctx| {
            let app = network::App::new(ctx, MultidayModel::empty());
            let states = vec![crate::network::Viewer::new_state(ctx, &app)];
            (app, states)
        });
    } else if n > 1 {
        panic!("You must specify one of --daily, --multiday, or --import-zip");
    }

    if let Some(path) = args.import_zip {
        widgetry::run(settings, move |ctx| {
            let app = ctx.loading_screen("initialize model", |ctx, timer| {
                let bytes = fs_err::read(path).unwrap();
                let models = Model::import_zip_bytes(bytes, timer).unwrap();
                for model in &models {
                    let save_model = base64::encode(abstutil::to_binary(model));
                    abstio::write_file(format!("data/output/{}.bin", model.main_date), save_model)
                        .unwrap();
                }

                let multiday = MultidayModel::new_from_daily_models(&models);
                abstio::write_file(
                    "data/output/multiday.bin".to_string(),
                    base64::encode(abstutil::to_binary(&multiday)),
                )
                .unwrap();

                network::App::new(ctx, multiday)
            });
            let states = vec![network::Viewer::new_state(ctx, &app)];
            (app, states)
        });
    } else if let Some(path) = args.daily {
        widgetry::run(settings, move |ctx| {
            let mut app = ctx.loading_screen("initialize model", |ctx, _timer| {
                let bytes = fs_err::read(path).unwrap();
                let decoded = base64::decode(bytes).unwrap();
                let model = abstutil::from_binary::<Model>(&decoded).unwrap();
                // TODO Experiments turned on
                //model.look_for_best_matches_by_pos_and_time();
                //model.supply_demand_matching().unwrap();
                //model.vehicles_with_few_stops().unwrap();
                App::new(ctx, model)
            });
            app.restore_savestate(ctx);
            let states = vec![replay::Replay::new_state(ctx, &app)];
            (app, states)
        });
    } else if let Some(path) = args.multiday {
        widgetry::run(settings, move |ctx| {
            let app = ctx.loading_screen("initialize model", |ctx, _timer| {
                let bytes = fs_err::read(path).unwrap();
                let decoded = base64::decode(bytes).unwrap();
                network::App::new(
                    ctx,
                    abstutil::from_binary::<MultidayModel>(&decoded).unwrap(),
                )
            });
            let states = vec![network::Viewer::new_state(ctx, &app)];
            app.restore_savestate(ctx);
            (app, states)
        });
    }
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

    // This only makes sense on native, with the same model used across different runs.
    // before_quit is never called on web, and web starts with an empty model.
    pub fn restore_savestate(&mut self, ctx: &mut EventCtx) {
        if let Ok(savestate) = abstio::maybe_read_json::<Savestate>(
            "data/save.json".to_string(),
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
    mode: SavestateMode,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SavestateMode {
    NetworkViewer,
    Replayer(Time, Option<VehicleID>),
}
