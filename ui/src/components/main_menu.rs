use anyhow::Result;
use serde::de::DeserializeOwned;
use widgetry::tools::PopupMsg;
use widgetry::{EventCtx, HorizontalAlignment, Line, Panel, Transition, VerticalAlignment, Widget};

use model::{Model, MultidayModel};

use crate::components::FileLoader;

pub struct MainMenu;

impl MainMenu {
    pub fn panel(ctx: &mut EventCtx) -> Panel {
        Panel::new_builder(Widget::col(vec![
            Line("Bus Spotting").small_heading().into_widget(ctx),
            Widget::row(vec![
                ctx.style().btn_outline.text("Load model").build_def(ctx),
                ctx.style().btn_outline.text("Import data").build_def(ctx),
            ]),
            Widget::placeholder(ctx, "contents"),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx)
    }

    pub fn on_click_network(
        ctx: &mut EventCtx,
        x: &str,
    ) -> Option<Transition<crate::network::App>> {
        match x {
            "Load model" => {
                return Some(load_model::<crate::network::App, MultidayModel>(
                    ctx,
                    Box::new(|ctx, app, model| {
                        *app = crate::network::App::new(ctx, model);
                    }),
                ));
            }
            "Import data" => {
                return Some(import_data::<crate::network::App>(
                    ctx,
                    Box::new(|ctx, app, multiday, _| {
                        *app = crate::network::App::new(ctx, multiday);
                    }),
                ));
            }
            _ => None,
        }
    }

    pub fn on_click_replay(ctx: &mut EventCtx, x: &str) -> Option<Transition<crate::App>> {
        match x {
            "Load model" => {
                return Some(load_model::<crate::App, Model>(
                    ctx,
                    Box::new(|ctx, app, model| {
                        *app = crate::App::new(ctx, model);
                    }),
                ));
            }
            "Import data" => {
                return Some(import_data::<crate::App>(
                    ctx,
                    Box::new(|ctx, app, _, mut singles| {
                        // Just load one of the days arbitrarily
                        *app = crate::App::new(ctx, singles.remove(0));
                    }),
                ));
            }
            _ => None,
        }
    }
}

fn load_model<A: 'static, M: 'static + DeserializeOwned>(
    ctx: &mut EventCtx,
    replace: Box<dyn Fn(&mut EventCtx, &mut A, M)>,
) -> Transition<A> {
    // TODO Restrict to .bin?
    Transition::Push(FileLoader::new_state(
        ctx,
        Box::new(move |ctx, app, maybe_bytes: Result<Option<Vec<u8>>>| {
            match maybe_bytes {
                Ok(Some(bytes)) => {
                    match base64::decode(bytes)
                        .map_err(|err| err.into())
                        .and_then(|bytes| abstutil::from_binary::<M>(&bytes))
                    {
                        Ok(model) => {
                            replace(ctx, app, model);
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

fn import_data<A: 'static>(
    ctx: &mut EventCtx,
    replace: Box<dyn Fn(&mut EventCtx, &mut A, MultidayModel, Vec<Model>)>,
) -> Transition<A> {
    // TODO Restrict to .zip?
    Transition::Push(FileLoader::new_state(
        ctx,
        Box::new(move |ctx, app, maybe_bytes: Result<Option<Vec<u8>>>| {
            match maybe_bytes {
                Ok(Some(bytes)) => ctx.loading_screen("import model", |ctx, timer| {
                    match Model::import_zip_bytes(bytes, timer) {
                        Ok(models) => {
                            for model in &models {
                                // TODO This silently fails in the browser unless we skip
                                // serializing vehicles. Apparently there are file size limits.
                                let save_model = base64::encode(abstutil::to_binary(model));
                                if let Err(err) = abstio::write_file(
                                    format!("data/output/{}.bin", model.main_date),
                                    save_model,
                                ) {
                                    error!("Couldn't save imported model: {err}");
                                }
                            }

                            let multiday = model::MultidayModel::new_from_daily_models(&models);
                            if let Err(err) = abstio::write_file(
                                "data/output/multiday.bin".to_string(),
                                base64::encode(abstutil::to_binary(&multiday)),
                            ) {
                                error!("Couldn't save imported model: {err}");
                            }

                            replace(ctx, app, multiday, models);
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
