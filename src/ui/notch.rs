use gpui::{prelude::*, *};

pub struct Notch {
    pub width: f32,
    pub height: f32,
}

impl Render for Notch {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("notch")
            .w(px(self.width))
            .h(px(self.height))
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x000000))
            .rounded_xl()
            .text_color(rgb(0xffffff))
            .child("Window Notch")
            .on_hover(cx.listener(|this, hovered, window, cx| {
                let (width, height) = if *hovered {
                    (540.0, 162.0)
                } else {
                    (180.0, 35.0)
                };

                if this.width == width && this.height == height {
                    return;
                }
                this.width = width;
                this.height = height;

                crate::platform::resize_notch(window, cx, width, height)
                    .expect("Failed to schedule notch resize");
                cx.notify();
            }))
    }
}
