use std::time::Duration;

use gpui::{prelude::*, *};

use crate::windows::media::{MediaAction, NowPlaying, SessionManager};

#[derive(Default)]
pub struct MusicPlayer {
    track: Option<NowPlaying>,
    refresh_task: Option<Task<()>>,
}

impl MusicPlayer {
    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if !visible {
            // Dropping the task stops the refresh loop while the notch is closed.
            self.refresh_task = None;
            return;
        }

        self.track = None;
        cx.notify();
        self.refresh_task = Some(cx.spawn(async move |this, cx| {
            let Some(manager) = SessionManager::connect().await else {
                return;
            };

            loop {
                let track = NowPlaying::read(&manager).await;
                if this
                    .update(cx, |player, cx| {
                        player.track = track;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }

                cx.background_executor().timer(Duration::from_secs(1)).await;
            }
        }));
    }

    fn button(
        &self,
        id: &'static str,
        icon: &'static str,
        action: MediaAction,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .w(px(42.0))
            .h(px(29.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .font_family("Segoe UI Symbol")
            .text_size(px(17.0))
            .text_color(rgb(if enabled { 0xf5f5f5 } else { 0x626262 }))
            .child(icon)
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0x222222)))
                    .on_click(cx.listener(move |player, _, _, cx| {
                        if let Some(track) = player.track.clone() {
                            cx.spawn(async move |_, _| {
                                let _ = track.send_action(action).await;
                            })
                            .detach();
                        }
                    }))
            })
    }
}

impl Render for MusicPlayer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let track = self.track.as_ref();
        let title = match track {
            Some(track) if !track.title.is_empty() => track.title.as_str(),
            Some(_) => "Unknown track",
            None => "Not Playing",
        };
        let artist = track.map_or("", |track| track.artist.as_str());
        let (elapsed, duration) = track.map_or((0, 0), |track| (track.elapsed, track.duration));
        let progress = elapsed as f32 / duration.max(1) as f32;
        let playing = track.is_some_and(|track| track.playing);
        let (play_icon, play_action, can_play_pause) = if playing {
            (
                "⏸",
                MediaAction::Pause,
                track.is_some_and(|track| track.can_pause),
            )
        } else {
            (
                "▶",
                MediaAction::Play,
                track.is_some_and(|track| track.can_play),
            )
        };

        div()
            .w(px(300.0))
            .flex()
            .flex_col()
            .gap(px(7.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .size(px(54.0))
                            .flex_none()
                            .rounded_lg()
                            .bg(rgb(0x1b1b1b))
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_family("Segoe UI Symbol")
                            .text_size(px(27.0))
                            .text_color(rgb(0x858585))
                            .child("♫"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title.to_owned()),
                            )
                            .when(!artist.is_empty(), |details| {
                                details.child(
                                    div()
                                        .truncate()
                                        .text_size(px(12.0))
                                        .text_color(rgb(0xaaaaaa))
                                        .child(artist.to_owned()),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .text_size(px(11.0))
                    .text_color(rgb(0x999999))
                    .child(format_time(elapsed))
                    .child(
                        div()
                            .flex_1()
                            .h(px(4.0))
                            .rounded_full()
                            .bg(rgb(0x242424))
                            .overflow_hidden()
                            .child(div().w(relative(progress)).h_full().bg(rgb(0xb0b0b0))),
                    )
                    .child(format!("-{}", format_time(duration - elapsed))),
            )
            .child(
                div()
                    .flex()
                    .justify_center()
                    .gap(px(5.0))
                    .child(self.button(
                        "previous",
                        "⏮",
                        MediaAction::Previous,
                        track.is_some_and(|track| track.can_previous),
                        cx,
                    ))
                    .child(self.button("play-pause", play_icon, play_action, can_play_pause, cx))
                    .child(self.button(
                        "next",
                        "⏭",
                        MediaAction::Next,
                        track.is_some_and(|track| track.can_next),
                        cx,
                    )),
            )
    }
}

fn format_time(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
