use chrono::Local;
use gpui::{prelude::*, *};

pub struct Clock;

impl Render for Clock {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::prelude::Context<Self>,
    ) -> impl gpui::prelude::IntoElement {
        div()
            .id("clock")
            .text_color(gpui::rgb(0xffffff))
            .text_size(gpui::px(24.0))
            .child(Local::now().format("%I:%M:%p").to_string())
    }
}
