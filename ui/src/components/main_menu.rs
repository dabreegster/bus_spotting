use anyhow::Result;
use widgetry::tools::PopupMsg;
use widgetry::{EventCtx, HorizontalAlignment, Line, Panel, VerticalAlignment, Widget};

use crate::components::FileLoader;
use crate::{App, Transition};

pub struct MainMenu;

impl MainMenu {
    pub fn panel(ctx: &mut EventCtx) -> Panel {
        Panel::new_builder(Widget::col(vec![
            Line("Bus Spotting").small_heading().into_widget(ctx),
            ctx.style().btn_outline.text("Import data").build_def(ctx),
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
            "Import data" => {
                return Some(import_data(ctx));
            }
            "Bus replay" => {
                return Some(Transition::Replace(
                    crate::bus_replay::BusReplay::new_state(ctx, app),
                ));
            }
            "Explore GTFS" => {
                return Some(Transition::Replace(crate::gtfs::ViewGTFS::new_state(
                    ctx, app,
                )));
            }
            _ => None,
        }
    }
}

fn import_data(ctx: &mut EventCtx) -> Transition {
    Transition::Push(FileLoader::new_state(
        ctx,
        Box::new(|ctx, _, maybe_bytes: Result<Option<Vec<u8>>>| {
            match maybe_bytes {
                Ok(Some(bytes)) => {
                    info!("got {} bytes", bytes.len());
                    Transition::Pop
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
