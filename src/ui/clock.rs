use std::time::Duration;

use chrono::{Local, Timelike};
use gpui::{prelude::*, *};

pub struct Clock {
    _refresh_task: Task<()>,
}

impl Clock {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let task = cx.spawn(async move |this, cx| {
            loop {
                let now = Local::now();
                let hour = now.hour();
                let minute = now.minute();
                let second = now.second();
                println!(
                    "Current second: {} at {} {} {} ",
                    second, hour, minute, second
                );
                let remaining: u64;
                if second != 0 {
                    remaining = 60 as u64 - second as u64;
                } else {
                    remaining = 60 as u64;
                }
                cx.background_executor()
                    .timer(Duration::from_secs(remaining))
                    .await;

                if this
                    .update(cx, |_, cx| {
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

        Self {
            _refresh_task: task,
        }
    }
}

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
