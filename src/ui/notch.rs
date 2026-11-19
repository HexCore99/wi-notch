use std::time::Duration;

use super::animation::SizeAnimation;
use super::music_player::MusicPlayer;
use super::settings::SettingsView;
use crate::appearance::{AppearanceSettings, Corners};
use crate::notch_geometry::EXPANDED_SIZE;
use crate::ui::clock::Clock;
use crate::windows::display::{monitors, selected_monitor};
use chrono::Local;
use gpui::{prelude::*, *};

pub struct Notch {
    pub width: f32,
    pub height: f32,

    compact_size: (f32, f32),
    expanded: bool,
    animation: Option<SizeAnimation>,
    _placement_watch: Task<()>,

    clock: Entity<Clock>,
    music_player: Entity<MusicPlayer>,
    appearance: Entity<AppearanceSettings>,
    settings_window: Option<WindowHandle<SettingsView>>,
}

impl Notch {
    pub fn new(
        width: f32,
        height: f32,
        appearance: Entity<AppearanceSettings>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&appearance, |_, _, cx| cx.notify()).detach();
        // A primary-display change can leave the notch off screen without a render.
        let placement_watch = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                if this
                    .update_in(cx, |notch, window, cx| notch.sync_placement(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            width,
            height,
            compact_size: (width, height),
            expanded: false,
            animation: None,
            _placement_watch: placement_watch,

            clock: cx.new(Clock::new),
            music_player: cx.new(|_| MusicPlayer::default()),
            appearance,
            settings_window: None,
        }
    }

    fn sync_placement(&self, window: &Window, cx: &App) {
        let settings = self.appearance.read(cx);
        let monitors = monitors();
        if let Some(monitor) = selected_monitor(&monitors, settings.monitor.as_deref())
            && let Err(error) = crate::windows::window::place_notch(
                window,
                cx,
                self.width,
                self.height,
                settings.top_gap,
                monitor,
            )
        {
            eprintln!("Failed to position the notch: {error}");
        }
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_window
            && handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            return;
        }

        self.settings_window = match SettingsView::open(self.appearance.clone(), cx) {
            Ok(handle) => Some(handle),
            Err(error) => {
                eprintln!("Failed to open settings: {error}");
                None
            }
        };
    }
}

impl Render for Notch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = self.appearance.read(cx);
        let background = settings.background.color();
        let rounded = settings.corners == Corners::Rounded;
        if let Some(animation) = &self.animation {
            let (width, height, finished) = animation.sample();
            self.width = width;
            self.height = height;

            if finished {
                self.animation = None;
            } else {
                window.request_animation_frame();
            }
        }

        self.sync_placement(window, cx);

        let expansion = ((self.height - self.compact_size.1)
            / (EXPANDED_SIZE.1 - self.compact_size.1))
            .clamp(0.0, 1.0);

        let date_opacity = ((expansion - 0.25) / 0.55).clamp(0.0, 1.0);
        let player_opacity = ((expansion - 0.6) / 0.35).clamp(0.0, 1.0);

        div()
            .id("notch")
            .w(px(self.width))
            .h(px(self.height))
            .relative()
            .flex()
            .flex_col()
            .items_center()
            .overflow_hidden()
            .bg(rgb(background))
            .when(rounded, |notch| notch.rounded_xl())
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
            .when(self.expanded && player_opacity > 0.0, |notch| {
                notch.child(
                    div()
                        .id("open-settings")
                        .absolute()
                        .top(px(5.0))
                        .right(px(12.0))
                        .size(px(25.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .opacity(player_opacity)
                        .cursor_pointer()
                        .hover(|style| style.bg(rgba(0xffffff20)))
                        .font_family("Segoe UI Symbol")
                        .text_size(px(16.0))
                        .child("⚙")
                        .on_click(cx.listener(|this, _, _, cx| this.open_settings(cx))),
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
