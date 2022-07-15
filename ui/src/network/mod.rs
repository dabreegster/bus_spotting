mod filters;
mod search;
mod stop;
mod variant;
mod viewer;

use abstutil::Timer;
use geom::Bounds;
use serde::{Deserialize, Serialize};
use widgetry::{Canvas, Color, EventCtx, GfxCtx, SharedAppState};

use model::MultidayModel;

use self::filters::Filters;
pub use self::viewer::Viewer;

pub struct App {
    model: MultidayModel,

    filters: Filters,

    // Avoid syncing when bounds match
    #[allow(unused)]
    mapbox_bounds: Bounds,
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
        };
        abstio::write_json("data/save_network.json".to_string(), &ss);
    }
}

pub type Transition = widgetry::Transition<App>;

impl App {
    pub fn new(ctx: &mut EventCtx, model: MultidayModel) -> Self {
        let bounds = &model.bounds;
        ctx.canvas.map_dims = (bounds.max_x, bounds.max_y);
        ctx.canvas.center_on_map_pt(bounds.center());

        Self {
            model,

            filters: Filters::new(),

            mapbox_bounds: Bounds::new(),
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
    pub fn restore_savestate(&self, ctx: &mut EventCtx) {
        if let Ok(savestate) = abstio::maybe_read_json::<Savestate>(
            "data/save_network.json".to_string(),
            &mut Timer::throwaway(),
        ) {
            ctx.canvas.cam_x = savestate.cam_x;
            ctx.canvas.cam_y = savestate.cam_y;
            ctx.canvas.cam_zoom = savestate.cam_zoom;
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Savestate {
    cam_x: f64,
    cam_y: f64,
    cam_zoom: f64,
}
