use anyhow::Result;
use widgetry::tools::FutureLoader;
use widgetry::{EventCtx, State, Transition};

// TODO Lift to widgetry::tools
pub struct FileLoader;

impl FileLoader {
    pub fn new_state<A: 'static>(
        ctx: &mut EventCtx,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A, Result<Option<Vec<u8>>>) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        let (_, outer_progress_rx) = futures_channel::mpsc::channel(1);
        let (_, inner_progress_rx) = futures_channel::mpsc::channel(1);
        FutureLoader::<A, Option<Vec<u8>>>::new_state(
            ctx,
            Box::pin(async move {
                let builder = rfd::AsyncFileDialog::new();
                let result = match builder.pick_file().await {
                    Some(file) => Some(file.read().await),
                    None => None,
                };
                let wrap: Box<dyn Send + FnOnce(&A) -> Option<Vec<u8>>> =
                    Box::new(move |_: &A| result);
                Ok(wrap)
            }),
            outer_progress_rx,
            inner_progress_rx,
            "Waiting for a file to be chosen",
            on_load,
        )
    }
}
