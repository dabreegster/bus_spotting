use anyhow::Result;
use widgetry::tools::PopupMsg;
use widgetry::{EventCtx, HorizontalAlignment, Line, Panel, VerticalAlignment, Widget};

use model::Model;

use crate::components::FileLoader;
use crate::{App, Transition};

pub struct MainMenu;

impl MainMenu {
    pub fn panel(ctx: &mut EventCtx) -> Panel {
        Panel::new_builder(Widget::col(vec![
            Line("Bus Spotting").small_heading().into_widget(ctx),
            Widget::row(vec![
                ctx.style().btn_outline.text("Load model").build_def(ctx),
                ctx.style().btn_outline.text("Import data").build_def(ctx),
            ]),
            // TODO Not sure how this should work yet
            Widget::row(vec![
                ctx.style().btn_solid.text("Bus replay").build_def(ctx),
                ctx.style().btn_solid.text("Explore GTFS").build_def(ctx),
            ]),
            Widget::placeholder(ctx, "contents"),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx)
    }

    pub fn on_click(ctx: &mut EventCtx, app: &App, x: &str) -> Option<Transition> {
        match x {
            "Load model" => {
                return Some(load_model(ctx));
            }
            "Import data" => {
                return Some(import_data(ctx));
            }
            "Bus replay" => {
                return Some(Transition::Replace(
                    crate::bus_replay::BusReplay::new_state(ctx, app),
                ));
            }
            "Explore GTFS" => {
                return Some(Transition::Replace(crate::network::Viewer::new_state(
                    ctx, app,
                )));
            }
            _ => None,
        }
    }
}

fn load_model(ctx: &mut EventCtx) -> Transition {
    // TODO Restrict to .bin?
    Transition::Push(FileLoader::new_state(
        ctx,
        Box::new(|ctx, app, maybe_bytes: Result<Option<Vec<u8>>>| {
            match maybe_bytes {
                Ok(Some(bytes)) => {
                    match base64::decode(bytes)
                        .map_err(|err| err.into())
                        .and_then(|bytes| abstutil::from_binary::<Model>(&bytes))
                    {
                        Ok(model) => {
                            *app = App::new(ctx, model);
                            Transition::Multi(vec![Transition::Pop, Transition::Recreate])
                        }
                        Err(err) => Transition::Replace(PopupMsg::new_state(
                            ctx,
                            "Error",
                            vec![err.to_string()],
                        )),
                    }
                }
                // User didn't pick a file
                Ok(None) => Transition::Pop,
                Err(err) => {
                    Transition::Replace(PopupMsg::new_state(ctx, "Error", vec![err.to_string()]))
                }
            }
        }),
    ))
}

fn import_data(ctx: &mut EventCtx) -> Transition {
    // TODO Restrict to .zip?
    Transition::Push(FileLoader::new_state(
        ctx,
        Box::new(|ctx, app, maybe_bytes: Result<Option<Vec<u8>>>| {
            match maybe_bytes {
                Ok(Some(bytes)) => ctx.loading_screen("import model", |ctx, timer| {
                    match Model::import_zip_bytes(bytes, timer) {
                        Ok(model) => {
                            // TODO This silently fails in the browser unless we skip serializing
                            // vehicles. Apparently there are file size limits.
                            let save_model = base64::encode(abstutil::to_binary(&model));
                            if let Err(err) =
                                abstio::write_file("data/output/model.bin".to_string(), save_model)
                            {
                                error!("Couldn't save imported model: {err}");
                            }

                            *app = App::new(ctx, model);
                            Transition::Multi(vec![Transition::Pop, Transition::Recreate])
                        }
                        Err(err) => Transition::Replace(PopupMsg::new_state(
                            ctx,
                            "Error",
                            vec![err.to_string()],
                        )),
                    }
                }),
                // User didn't pick a file
                Ok(None) => Transition::Pop,
                Err(err) => {
                    Transition::Replace(PopupMsg::new_state(ctx, "Error", vec![err.to_string()]))
                }
            }
        }),
    ))
}
