use super::animation::SizeAnimation;
use super::music_player::MusicPlayer;
use crate::ui::clock::Clock;
use chrono::Local;
use gpui::{prelude::*, *};

const EXPANDED_SIZE: (f32, f32) = (340.0, 162.0);

pub struct Notch {
    pub width: f32,
    pub height: f32,

    compact_size: (f32, f32),
    expanded: bool,
    animation: Option<SizeAnimation>,

    clock: Entity<Clock>,
    music_player: Entity<MusicPlayer>,
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
            music_player: cx.new(|_| MusicPlayer::default()),
        }
    }
}

impl Render for Notch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(animation) = &self.animation {
            let (width, height, finished) = animation.sample();
            self.width = width;
            self.height = height;

            crate::windows::window::resize_notch(window, cx, width, height)
                .expect("Failed to resize the notch");

            if finished {
                self.animation = None;
            } else {
                window.request_animation_frame();
            }
        }

        let expansion = ((self.height - self.compact_size.1)
            / (EXPANDED_SIZE.1 - self.compact_size.1))
            .clamp(0.0, 1.0);

        let date_opacity = ((expansion - 0.25) / 0.55).clamp(0.0, 1.0);
        let player_opacity = ((expansion - 0.6) / 0.35).clamp(0.0, 1.0);

        div()
            .id("notch")
            .w(px(self.width))
            .h(px(self.height))
            .flex()
            .flex_col()
            .items_center()
            .overflow_hidden()
            .bg(rgb(0x000000))
            .rounded_xl()
            .text_color(rgb(0xffffff))
            .font_weight(FontWeight(600.0))
            .child(
                div()
                    .h(px(self.compact_size.1))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.0 * date_opacity))
                    .when(date_opacity > 0.0, |header| {
                        header.child(
                            div()
                                .w(px(104.0 * date_opacity))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .opacity(date_opacity)
                                .text_size(px(12.0))
                                .child(Local::now().format("%a %d %b %Y").to_string()),
                        )
                    })
                    .child(self.clock.clone()),
            )
            .when(player_opacity > 0.0, |notch| {
                notch.child(
                    div()
                        .opacity(player_opacity)
                        .child(self.music_player.clone()),
                )
            })
            .on_hover(cx.listener(|this, hovered, _window, cx| {
                if this.expanded == *hovered {
                    return;
                }
                this.expanded = *hovered;
                this.music_player.update(cx, |player, cx| {
                    player.set_visible(*hovered, cx);
                });
                let target_size = if *hovered {
                    EXPANDED_SIZE
                } else {
                    this.compact_size
                };

                this.animation = Some(SizeAnimation::new((this.width, this.height), target_size));

                cx.notify();
            }))
    }
}
