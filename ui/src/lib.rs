#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod components;
mod daily;
mod multiday;

use geom::{Bounds, GPSBounds};
use structopt::StructOpt;
use widgetry::{EventCtx, Settings};

use model::{DailyModel, MultidayModel};

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

// This is a bit complex -- based on the input args, enter daily or multiday mode. They're two
// totally separate UIs; they just live in one crate and are deployed as one binary for now.
fn run(settings: Settings) {
    abstutil::logger::setup();

    let args = Args::from_iter(abstutil::cli_args());
    let n = [&args.daily, &args.multiday, &args.import_zip]
        .iter()
        .filter(|x| x.is_some())
        .count();
    if n == 0 {
        // Empty multiday view
        widgetry::run(settings, |ctx| {
            let app = multiday::App::new(ctx, MultidayModel::empty());
            let states = vec![crate::multiday::Viewer::new_state(ctx, &app)];
            (app, states)
        });
    } else if n > 1 {
        panic!("You must specify one of --daily, --multiday, or --import-zip");
    }

    if let Some(path) = args.import_zip {
        widgetry::run(settings, move |ctx| {
            let app = ctx.loading_screen("initialize model", |ctx, timer| {
                let bytes = fs_err::read(path).unwrap();
                let models = DailyModel::import_zip_bytes(bytes, timer).unwrap();
                for model in &models {
                    let save_model = base64::encode(abstutil::to_binary(model));
                    abstio::write_file(format!("data/output/{}.bin", model.date), save_model)
                        .unwrap();
                }

                let multiday = MultidayModel::new_from_daily_models(&models);
                abstio::write_file(
                    "data/output/multiday.bin".to_string(),
                    base64::encode(abstutil::to_binary(&multiday)),
                )
                .unwrap();

                multiday::App::new(ctx, multiday)
            });
            let states = vec![multiday::Viewer::new_state(ctx, &app)];
            (app, states)
        });
    } else if let Some(path) = args.daily {
        widgetry::run(settings, move |ctx| {
            let mut app = ctx.loading_screen("initialize model", |ctx, _timer| {
                let bytes = fs_err::read(path).unwrap();
                let decoded = base64::decode(bytes).unwrap();
                let model = abstutil::from_binary::<DailyModel>(&decoded).unwrap();
                // TODO Experiments turned on
                //model.look_for_best_matches_by_pos_and_time();
                //model.supply_demand_matching().unwrap();
                //model.vehicles_with_few_stops().unwrap();
                daily::App::new(ctx, model)
            });
            app.restore_savestate(ctx);
            let states = vec![daily::Replay::new_state(ctx, &app)];
            (app, states)
        });
    } else if let Some(path) = args.multiday {
        widgetry::run(settings, move |ctx| {
            let app = ctx.loading_screen("initialize model", |ctx, _timer| {
                let bytes = fs_err::read(path).unwrap();
                let decoded = base64::decode(bytes).unwrap();
                multiday::App::new(
                    ctx,
                    abstutil::from_binary::<MultidayModel>(&decoded).unwrap(),
                )
            });
            let states = vec![multiday::Viewer::new_state(ctx, &app)];
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
    fn sync_maplibre_canvas(lon1: f64, lat1: f64, lon2: f64, lat2: f64);
}

pub struct MaplibreSync(Bounds);

impl MaplibreSync {
    pub fn new() -> Self {
        Self(Bounds::new())
    }

    #[allow(unused)]
    pub fn sync(&mut self, ctx: &mut EventCtx, gps_bounds: &GPSBounds) {
        #[cfg(target_arch = "wasm32")]
        {
            // This method is usually called for every single event, but the camera hasn't always
            // moved
            let bounds = ctx.canvas.get_screen_bounds();
            if self.0 == bounds {
                return;
            }
            self.0 = bounds;

            let pt1 = geom::Pt2D::new(bounds.min_x, bounds.min_y).to_gps(gps_bounds);
            let pt2 = geom::Pt2D::new(bounds.max_x, bounds.max_y).to_gps(gps_bounds);
            sync_maplibre_canvas(pt1.x(), pt1.y(), pt2.x(), pt2.y());
        }
    }
}
