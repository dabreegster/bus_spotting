use std::collections::BTreeMap;

use widgetry::{
    ButtonBuilder, DrawBaselayer, EventCtx, GfxCtx, Line, Outcome, Panel, State, Transition, Widget,
};

// TODO Move to widgetry if this pattern holds up

pub struct PageBuilder<T> {
    buttons: BTreeMap<String, T>,
}

impl<T: 'static> PageBuilder<T> {
    pub fn new() -> Self {
        Self {
            buttons: BTreeMap::new(),
        }
    }

    pub fn btn_data<'a, 'c>(
        &mut self,
        ctx: &mut EventCtx,
        btn: ButtonBuilder<'a, 'c>,
        data: T,
    ) -> Widget {
        self.buttons
            .insert(btn.get_action().unwrap().to_string(), data);
        btn.build_def(ctx)
    }

    // TODO Could take a panel
    pub fn build<A: 'static>(
        self,
        ctx: &mut EventCtx,
        title: &str,
        layout: Widget,
        cb: Box<dyn Fn(&mut EventCtx, &mut A, T) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        Box::new(PageState {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line(title).small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                layout,
            ]))
            .build(ctx),
            buttons: self.buttons,
            cb,
        })
    }
}

pub struct PageState<A, T> {
    panel: Panel,
    buttons: BTreeMap<String, T>,
    cb: Box<dyn Fn(&mut EventCtx, &mut A, T) -> Transition<A>>,
}

impl<A: 'static, T: 'static> State<A> for PageState<A, T> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            if x == "close" {
                return Transition::Pop;
            }
            // The callback must destroy this PageState
            match self.buttons.remove(&x) {
                Some(data) => {
                    return (self.cb)(ctx, app, data);
                }
                None => {
                    panic!("Page doesn't have a handler for {x}; was btn_data called?");
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &A) {
        self.panel.draw(g);
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }
}
