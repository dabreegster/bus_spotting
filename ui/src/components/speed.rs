use geom::{Duration, Time};
use widgetry::{
    include_labeled_bytes, Choice, ControlState, EdgeInsets, EventCtx, GfxCtx, HorizontalAlignment,
    Key, Line, Outcome, Panel, PersistentSplit, ScreenDims, Slider, Text, VerticalAlignment,
    Widget,
};

use crate::App;

// TODO Maybe the component pattern is to
//
// 1) Not even touch App -- take in data we need from it
// 2) Explicitly return stuff we did
//
// and make the callers
//
// and in this case, maybe even split things into smaller pieces:
// - the time display (dont share it)
// - the play/pause
// - the speed
// - the time increment bit

pub struct TimeControls {
    pub panel: Panel,
    time: Time,
    paused: bool,
    setting: SpeedSetting,
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum SpeedSetting {
    /// 1 sim second per real second
    Realtime,
    /// 5 sim seconds per real second
    Fast,
    /// 30 sim seconds per real second
    Faster,
    /// 1 sim hour per real second
    Fastest,
}

impl TimeControls {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Self {
        let mut time = Self {
            panel: Panel::new_builder(Widget::col(vec![
                Slider::area(
                    ctx,
                    0.15 * ctx.canvas.window_width,
                    app.time.to_percent(end_of_day()),
                    "time slider",
                ),
                Widget::placeholder(ctx, "clock"),
                Widget::placeholder(ctx, "controls"),
                Widget::placeholder(ctx, "stats"),
            ]))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Bottom)
            .build(ctx),
            time: app.time,
            paused: false,
            setting: SpeedSetting::Realtime,
        };
        time.update_controls(ctx, app);
        time
    }

    fn update_controls(&mut self, ctx: &mut EventCtx, app: &App) {
        self.on_time_change(ctx);

        let mut row = Vec::new();
        row.push({
            let button = ctx
                .style()
                .btn_plain
                .icon_bytes(include_labeled_bytes!("../../assets/triangle.svg"))
                .hotkey(Key::Space);

            Widget::custom_row(vec![if self.paused {
                button.build_widget(ctx, "play")
            } else {
                button
                    .image_bytes(include_labeled_bytes!("../../assets/pause.svg"))
                    .build_widget(ctx, "pause")
            }])
            .margin_right(16)
        });

        row.push(
            Widget::custom_row(
                vec![
                    (SpeedSetting::Realtime, "real-time speed"),
                    (SpeedSetting::Fast, "5x speed"),
                    (SpeedSetting::Faster, "30x speed"),
                    (SpeedSetting::Fastest, "3600x speed"),
                ]
                .into_iter()
                .map(|(s, label)| {
                    let mut txt = Text::from(Line(label).small());
                    txt.extend(Text::tooltip(ctx, Key::LeftArrow, "slow down"));
                    txt.extend(Text::tooltip(ctx, Key::RightArrow, "speed up"));

                    let mut triangle_btn = ctx
                        .style()
                        .btn_plain
                        .btn()
                        .image_bytes(include_labeled_bytes!("../../assets/triangle.svg"))
                        .image_dims(ScreenDims::new(16.0, 26.0))
                        .tooltip(txt)
                        .padding(EdgeInsets {
                            top: 8.0,
                            bottom: 8.0,
                            left: 3.0,
                            right: 3.0,
                        });

                    if s == SpeedSetting::Realtime {
                        triangle_btn = triangle_btn.padding_left(10.0);
                    }
                    if s == SpeedSetting::Fastest {
                        triangle_btn = triangle_btn.padding_right(10.0);
                    }

                    if self.setting < s {
                        triangle_btn = triangle_btn
                            .image_color(ctx.style().btn_outline.fg_disabled, ControlState::Default)
                    }

                    triangle_btn.build_widget(ctx, label)
                })
                .collect(),
            )
            .margin_right(16),
        );

        row.push(
            PersistentSplit::widget(
                ctx,
                "step forwards",
                app.time_increment,
                Key::M,
                vec![
                    Choice::new("+1h", Duration::hours(1)),
                    Choice::new("+30m", Duration::minutes(30)),
                    Choice::new("+10m", Duration::minutes(10)),
                    Choice::new("+0.1s", Duration::seconds(0.1)),
                ],
            )
            .margin_right(16),
        );

        row.push(
            ctx.style()
                .btn_plain
                .icon_bytes(include_labeled_bytes!("../../assets/reset.svg"))
                .hotkey(Key::X)
                .build_widget(ctx, "reset to midnight"),
        );

        self.panel.replace(ctx, "controls", Widget::custom_row(row));
    }

    fn on_time_change(&mut self, ctx: &mut EventCtx) {
        let clock = Text::from(Line(self.time.ampm_tostring()).big_monospaced()).into_widget(ctx);
        self.panel.replace(ctx, "clock", clock);

        self.panel
            .slider_mut("time slider")
            .set_percent(ctx, self.time.to_percent(end_of_day()));
    }

    // May update app.time
    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) {
        if self.time != app.time {
            self.time = app.time;
            self.on_time_change(ctx);
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "real-time speed" => {
                    self.setting = SpeedSetting::Realtime;
                    self.update_controls(ctx, app);
                }
                "5x speed" => {
                    self.setting = SpeedSetting::Fast;
                    self.update_controls(ctx, app);
                }
                "30x speed" => {
                    self.setting = SpeedSetting::Faster;
                    self.update_controls(ctx, app);
                }
                "3600x speed" => {
                    self.setting = SpeedSetting::Fastest;
                    self.update_controls(ctx, app);
                }
                "play" => {
                    self.paused = false;
                    self.update_controls(ctx, app);
                }
                "pause" => {
                    self.pause(ctx, app);
                }
                "reset to midnight" => {
                    app.time = Time::START_OF_DAY;
                }
                "step forwards" => {
                    app.time += app.time_increment;
                }
                _ => unreachable!(),
            },
            Outcome::Changed(x) => match x.as_ref() {
                "step forwards" => {
                    app.time_increment = self.panel.persistent_split_value("step forwards");
                }
                "time slider" => {
                    app.time =
                        end_of_day().percent_of(self.panel.slider("time slider").get_percent());
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        if ctx.input.pressed(Key::LeftArrow) {
            match self.setting {
                SpeedSetting::Realtime => self.pause(ctx, app),
                SpeedSetting::Fast => {
                    self.setting = SpeedSetting::Realtime;
                    self.update_controls(ctx, app);
                }
                SpeedSetting::Faster => {
                    self.setting = SpeedSetting::Fast;
                    self.update_controls(ctx, app);
                }
                SpeedSetting::Fastest => {
                    self.setting = SpeedSetting::Faster;
                    self.update_controls(ctx, app);
                }
            }
        }
        if ctx.input.pressed(Key::RightArrow) {
            match self.setting {
                SpeedSetting::Realtime => {
                    if self.paused {
                        self.paused = false;
                    } else {
                        self.setting = SpeedSetting::Fast;
                    }
                    self.update_controls(ctx, app);
                }
                SpeedSetting::Fast => {
                    self.setting = SpeedSetting::Faster;
                    self.update_controls(ctx, app);
                }
                SpeedSetting::Faster => {
                    self.setting = SpeedSetting::Fastest;
                    self.update_controls(ctx, app);
                }
                SpeedSetting::Fastest => {}
            }
        }

        if !self.paused {
            if let Some(real_dt) = ctx.input.nonblocking_is_update_event() {
                ctx.input.use_update_event();
                let multiplier = match self.setting {
                    SpeedSetting::Realtime => 1.0,
                    SpeedSetting::Fast => 5.0,
                    SpeedSetting::Faster => 30.0,
                    SpeedSetting::Fastest => 3600.0,
                };
                let dt = multiplier * real_dt;
                app.time += dt;
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.panel.draw(g);
    }

    pub fn pause(&mut self, ctx: &mut EventCtx, app: &App) {
        if !self.paused {
            self.paused = true;
            self.update_controls(ctx, app);
        }
    }

    pub fn _resume(&mut self, ctx: &mut EventCtx, app: &App, setting: SpeedSetting) {
        if self.paused || self.setting != setting {
            self.paused = false;
            self.setting = setting;
            self.update_controls(ctx, app);
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }
}

fn end_of_day() -> Time {
    Time::START_OF_DAY + Duration::hours(24)
}
