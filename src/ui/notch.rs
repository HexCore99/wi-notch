use super::animation::SizeAnimation;
use chrono::Local;
use gpui::{prelude::*, *};

pub struct Notch {
    pub width: f32,
    pub height: f32,

    compact_size: (f32, f32),
    expanded: bool,
    animation: Option<SizeAnimation>,
}

impl Notch {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            compact_size: (width, height),
            expanded: false,
            animation: None,
        }
    }
}

impl Render for Notch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(animation) = &self.animation {
            let (width, height, finished) = animation.sample();
            self.width = width;
            self.height = height;

            crate::platform::resize_notch(window, cx, width, height)
                .expect("Failed to resize the notch");

            if finished {
                self.animation = None;
            } else {
                window.request_animation_frame();
            }
        }
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
            .child(Local::now().format("%I:%M:%p").to_string())
            .on_hover(cx.listener(|this, hovered, _window, cx| {
                if this.expanded == *hovered {
                    return;
                }
                this.expanded = *hovered;
                let target_size = if *hovered {
                    (540.0, 162.0)
                } else {
                    this.compact_size
                };

                this.animation = Some(SizeAnimation::new((this.width, this.height), target_size));

                cx.notify();
            }))
    }
}
