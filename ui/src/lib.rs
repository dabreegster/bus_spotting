#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

mod bus_replay;
mod components;
mod gtfs;

use abstutil::Timer;
use anyhow::Result;
use geom::{Duration, Time};
use structopt::StructOpt;
use widgetry::{EventCtx, Settings, SharedAppState};

use model::Model;

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
            return abstio::maybe_read_binary::<Model>(path, timer);
        }
        if self.import_zip.is_none() {
            return Ok(Model::empty());
        }
        let bytes = fs_err::read(self.import_zip.take().unwrap())?;
        let model = Model::import_zip_bytes(bytes, timer)?;
        // TODO In the browser, make them download the file
        abstio::write_binary("data/output/model.bin".to_string(), &model);
        Ok(model)
    }
}

fn run(settings: Settings) {
    abstutil::logger::setup();

    let args = Args::from_iter(abstutil::cli_args());

    widgetry::run(settings, move |ctx| {
        let model = ctx.loading_screen("initialize model", |_, timer| args.load(timer).unwrap());
        let app = App::new(ctx, model);
        let states = vec![crate::gtfs::ViewGTFS::new_state(ctx, &app)];
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

pub struct App {
    model: Model,
    // TODO Maybe this is per-mode state
    time: Time,
    time_increment: Duration,
}

impl SharedAppState for App {}

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
        }
    }
}
