use super::animation::SizeAnimation;
use crate::ui::clock::Clock;
use chrono::Local;
use gpui::{prelude::*, *};

pub struct Notch {
    pub width: f32,
    pub height: f32,

    compact_size: (f32, f32),
    expanded: bool,
    animation: Option<SizeAnimation>,

    clock: Entity<Clock>,
}

impl Notch {
    pub fn new(width: f32, height: f32, cx: &mut Context<Self>) -> Self {
        Self {
            width,
            height,
            compact_size: (width, height),
            expanded: false,
            animation: None,

            clock: cx.new(Clock::new),
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

        let expansion =
            ((self.height - self.compact_size.1) / (162.0 - self.compact_size.1)).clamp(0.0, 1.0);

        let date_opacity = ((expansion - 0.25) / 0.55).clamp(0.0, 1.0);

        div()
            .id("notch")
            .w(px(self.width))
            .h(px(self.height))
            .flex()
            // .items_center()
            .justify_center()
            .bg(rgb(0x000000))
            .rounded_xl()
            .text_color(rgb(0xffffff))
            .child(self.clock.clone())
            .child(
                div()
                    .ml(px(12.0 * date_opacity))
                    .w(px(100.0 * date_opacity))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .opacity(date_opacity)
                    .child(Local::now().date_naive().to_string()),
            )
            .on_hover(cx.listener(|this, hovered, _window, cx| {
                if this.expanded == *hovered {
                    return;
                }
                this.expanded = *hovered;
                let target_size = if *hovered {
                    (340.0, 162.0)
                } else {
                    this.compact_size
                };

                this.animation = Some(SizeAnimation::new((this.width, this.height), target_size));

                cx.notify();
            }))
    }
}
