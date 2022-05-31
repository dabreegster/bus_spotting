#[macro_use]
extern crate anyhow;

mod speed;
mod viewer;

use abstutil::Timer;
use anyhow::Result;
use geom::{Duration, Time};
use structopt::StructOpt;
use widgetry::{Settings, SharedAppState};

use model::Model;

#[derive(StructOpt)]
struct Args {
    /// The path to a previously built and serialized model
    #[structopt(long)]
    model: Option<String>,
    /// The path to an AVL CSV file
    #[structopt(long)]
    avl: Option<String>,
    /// The path to a GTFS directory
    #[structopt(long)]
    gtfs: Option<String>,
}

impl Args {
    fn load(mut self, timer: &mut Timer) -> Result<Model> {
        if let Some(path) = self.model.take() {
            if self.avl.is_some() || self.gtfs.is_some() {
                bail!("If --model is specified, nothing will be imported");
            }
            return abstio::maybe_read_binary::<Model>(path, timer);
        }
        if self.avl.is_none() && self.gtfs.is_none() {
            return Ok(Model::empty());
        }
        if self.avl.is_none() || self.gtfs.is_none() {
            bail!("Both --avl and --gtfs needed to import a model");
        }
        let model = Model::import(&self.avl.take().unwrap(), &self.gtfs.take().unwrap())?;
        // TODO Don't save to a fixed path; maybe use the date
        abstio::write_binary("model.bin".to_string(), &model);
        Ok(model)
    }
}

fn run(settings: Settings) {
    abstutil::logger::setup();

    let args = Args::from_iter(abstutil::cli_args());

    widgetry::run(settings, move |ctx| {
        let model = ctx.loading_screen("initialize model", |_, timer| args.load(timer).unwrap());

        let bounds = &model.bounds;
        ctx.canvas.map_dims = (bounds.max_x, bounds.max_y);
        ctx.canvas.center_on_map_pt(bounds.center());

        let app = App {
            model,
            time: Time::START_OF_DAY,
            time_increment: Duration::minutes(10),
        };
        let states = vec![crate::viewer::Viewer::new(ctx, &app)];
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
    time: Time,
    time_increment: Duration,
}

impl SharedAppState for App {}
