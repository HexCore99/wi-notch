use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use super::animation::SizeAnimation;
use super::music_player::MusicPlayer;
use super::notifications::{
    Notifications, PREVIEW_HEIGHT, render_artwork, render_icon, render_icon_pixels,
};
use super::settings::SettingsView;
use crate::appearance::{AppearanceSettings, Corners};
use crate::notch_geometry::{COLLAPSED_SIZE, EXPANDED_SIZE};
use crate::ui::clock::Clock;
use crate::windows::app_icons::{read_registered_icon, read_shell_icon};
use crate::windows::display::{Monitor, monitors, selected_monitor};
use crate::windows::media::SessionManager;
use crate::windows::notifications::{
    Access, Listener, Snapshot, Toast, ToastKey, open_notification, read_artwork, read_icon,
};
use chrono::Local;
use gpui::{prelude::*, *};

const DATE_REVEAL_SECONDS: f32 = 0.38;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Compact,
    Collapsed,
    Music,
    Notifications,
    Preview(ToastKey),
}

pub struct Notch {
    pub width: f32,
    pub height: f32,

    compact_size: (f32, f32),
    hovered: bool,
    playing: bool,
    always_minimal: bool,
    idle_since: Option<Instant>,
    mode: Mode,
    target_size: (f32, f32),
    animation: Option<SizeAnimation>,
    date_reveal: f32,
    date_reveal_frame: Option<Instant>,
    displays: Vec<Monitor>,
    _placement_watch: Task<()>,
    _playback_watch: Task<()>,
    _notification_watch: Option<Task<()>>,
    preview_timeout: Option<Task<()>>,
    opening_notification: bool,

    clock: Entity<Clock>,
    music_player: Entity<MusicPlayer>,
    notifications: Notifications,
    appearance: Entity<AppearanceSettings>,
    settings_window: Option<WindowHandle<SettingsView>>,
}

impl Notch {
    pub fn new(
        width: f32,
        height: f32,
        appearance: Entity<AppearanceSettings>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe_in(&appearance, window, |this, _, window, cx| {
            this.sync_idle_policy(cx);
            this.sync_placement(window, cx);
            cx.notify();
        })
        .detach();
        cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() && this.mode == Mode::Notifications {
                this.set_mode(Mode::Compact, cx);
            }
        })
        .detach();
        // A primary-display change can leave the notch off screen without a render.
        let placement_watch = cx.spawn_in(window, async move |this, cx| {
            let executor = cx.background_executor().clone();
            loop {
                executor.timer(Duration::from_millis(500)).await;
                let displays = executor.spawn(async { monitors() }).await;
                if this
                    .update_in(cx, |notch, window, cx| {
                        notch.displays = displays;
                        notch.sync_placement(window, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let playback_watch = cx.spawn(async move |this, cx| {
            let mut manager = None;
            loop {
                if manager.is_none() {
                    manager = SessionManager::connect().await;
                }
                let playing = manager.as_ref().is_some_and(SessionManager::is_playing);
                if this
                    .update(cx, |notch, cx| {
                        notch.playing = playing;
                        notch.sync_idle_policy(cx);
                    })
                    .is_err()
                {
                    break;
                }
                cx.background_executor().timer(Duration::from_secs(1)).await;
            }
        });
        let listener = Listener::connect();
        let access = listener
            .as_ref()
            .map_or_else(|access| *access, Listener::access);
        let notification_watch = listener.is_ok().then(|| {
            cx.spawn_in(window, async move |this, cx| {
                let executor = cx.background_executor().clone();
                loop {
                    let (access, snapshot) = executor.spawn(async { Listener::poll() }).await;
                    if this
                        .update_in(cx, |notch, _, cx| {
                            notch.sync_notifications(access, snapshot, cx)
                        })
                        .is_err()
                    {
                        break;
                    }
                    executor.timer(Duration::from_secs(1)).await;
                }
            })
        });
        let always_minimal = appearance.read(cx).always_minimal;
        let compact_size = (width, height);
        let (mode, (width, height)) = if always_minimal {
            (Mode::Collapsed, COLLAPSED_SIZE)
        } else {
            (Mode::Compact, compact_size)
        };
        let mut notch = Self {
            width,
            height,
            compact_size,
            hovered: false,
            playing: false,
            always_minimal,
            idle_since: None,
            mode,
            target_size: (width, height),
            animation: None,
            date_reveal: 0.0,
            date_reveal_frame: None,
            displays: monitors(),
            _placement_watch: placement_watch,
            _playback_watch: playback_watch,
            _notification_watch: notification_watch,
            preview_timeout: None,
            opening_notification: false,

            clock: cx.new(Clock::new),
            music_player: cx.new(|_| MusicPlayer::default()),
            notifications: Notifications::new(access),
            appearance,
            settings_window: None,
        };
        notch.update_idle_start(cx);
        notch.sync_placement(window, cx);
        notch
    }

    fn hover_mode(&self) -> Mode {
        if self.hovered {
            Mode::Music
        } else if self.always_minimal {
            Mode::Collapsed
        } else {
            Mode::Compact
        }
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        let mode = if self.always_minimal && !(self.hovered && mode == Mode::Notifications) {
            self.hover_mode()
        } else {
            mode
        };
        if self.mode == mode {
            return;
        }
        let music_visible = mode == Mode::Music;
        if (self.mode == Mode::Music) != music_visible {
            self.music_player
                .update(cx, |player, cx| player.set_visible(music_visible, cx));
        }
        self.mode = mode;
        if !matches!(mode, Mode::Preview(_)) {
            self.preview_timeout = None;
        }
        self.update_idle_start(cx);
        cx.notify();
    }

    fn update_idle_start(&mut self, cx: &App) {
        if self.mode == Mode::Compact
            && !self.hovered
            && !self.playing
            && self.appearance.read(cx).auto_minimal
        {
            self.idle_since.get_or_insert_with(Instant::now);
        } else {
            self.idle_since = None;
        }
    }

    fn sync_idle_policy(&mut self, cx: &mut Context<Self>) {
        let settings = self.appearance.read(cx);
        let was_always_minimal = self.always_minimal;
        self.always_minimal = settings.always_minimal;
        let auto_minimal = settings.auto_minimal;
        let delay = Duration::from_secs(settings.idle_collapse_seconds().into());

        if self.always_minimal {
            // Apply the preference to the current panel, including when it changes in Settings.
            self.set_mode(self.mode, cx);
            return;
        }

        if self.mode == Mode::Collapsed && (was_always_minimal || !auto_minimal || self.playing) {
            self.set_mode(self.hover_mode(), cx);
        }

        self.update_idle_start(cx);
        if self
            .idle_since
            .is_some_and(|since| since.elapsed() >= delay)
        {
            self.set_mode(Mode::Collapsed, cx);
        }
    }

    fn toggle_notifications(&mut self, cx: &mut Context<Self>) {
        if self.mode == Mode::Notifications {
            self.set_mode(self.hover_mode(), cx);
        } else {
            self.notifications.open();
            self.set_mode(Mode::Notifications, cx);
        }
    }

    fn open_notification_app(&mut self, key: ToastKey, cx: &mut Context<Self>) {
        if self.opening_notification {
            return;
        }
        let Some(toast) = self.notifications.find(key).cloned() else {
            return;
        };
        self.opening_notification = true;
        cx.spawn(async move |this, cx| {
            let executor = cx.background_executor().clone();
            let result = open_notification(toast, &executor).await;
            let _ = this.update(cx, |this, cx| {
                this.opening_notification = false;
                match result {
                    Ok(()) if this.mode == Mode::Notifications => this.set_mode(Mode::Compact, cx),
                    Ok(()) => {}
                    Err(error) => {
                        eprintln!("Failed to open the notification's source app: {error}");
                        this.notifications.action_error =
                            Some((key, "Windows couldn’t open this app."));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn dismiss_notification(&mut self, key: ToastKey, cx: &mut Context<Self>) {
        if self.notifications.find(key).is_none() {
            return;
        }
        let Ok(listener) = Listener::connect() else {
            return;
        };
        match listener.dismiss(key) {
            Ok(()) => {
                self.notifications.dismiss(key);
                if self.mode == Mode::Preview(key) {
                    self.set_mode(self.hover_mode(), cx);
                }
            }
            Err(error) => {
                eprintln!("Failed to dismiss a Windows notification: {error}");
                self.notifications.action_error =
                    Some((key, "Windows couldn’t dismiss this notification."));
            }
        }
        cx.notify();
    }

    fn request_notification_access(&mut self, cx: &mut Context<Self>) {
        if self.notifications.requesting {
            return;
        }
        self.notifications.requesting = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let access = match Listener::connect() {
                Ok(listener) => listener.request_access().await,
                Err(access) => access,
            };
            let _ = this.update(cx, |notch, cx| {
                notch.notifications.requesting = false;
                notch.sync_notifications(access, None, cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn sync_notifications(
        &mut self,
        access: Access,
        snapshot: Option<Snapshot>,
        cx: &mut Context<Self>,
    ) {
        let access_changed = self
            .notifications
            .set_access(access, |image| cx.drop_image(image, None));
        if let Some(snapshot) = snapshot {
            self.sync_notification_artwork(&snapshot.toasts, cx);
            let (changed, newest) = self
                .notifications
                .sync(snapshot.toasts, self.mode == Mode::Notifications);
            self.sync_notification_icons(snapshot.apps, cx);

            if let Some(key) = newest
                && !self.always_minimal
                && matches!(
                    self.mode,
                    Mode::Compact | Mode::Collapsed | Mode::Preview(_)
                )
            {
                self.set_mode(Mode::Preview(key), cx);
                self.preview_timeout = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_secs(4)).await;
                    let _ = this.update(cx, |notch, cx| {
                        if notch.mode == Mode::Preview(key) {
                            notch.set_mode(notch.hover_mode(), cx);
                        }
                    });
                }));
            }
            if changed {
                cx.notify();
            }
        }

        if let Mode::Preview(key) = self.mode
            && self.notifications.find(key).is_none()
        {
            self.set_mode(self.hover_mode(), cx);
        }
        if access_changed {
            cx.notify();
        }
    }

    fn sync_notification_artwork(&mut self, toasts: &[Toast], cx: &mut Context<Self>) {
        let keys: HashSet<_> = toasts.iter().map(|toast| toast.key).collect();
        self.notifications.artwork.retain(|key, image| {
            if keys.contains(key) {
                return true;
            }
            if let Some(image) = image.take() {
                cx.drop_image(image, None);
            }
            false
        });

        let pending: Vec<_> = toasts
            .iter()
            .filter(|toast| !self.notifications.artwork.contains_key(&toast.key))
            .cloned()
            .collect();
        if pending.is_empty() {
            return;
        }
        self.notifications
            .artwork
            .extend(pending.iter().map(|toast| (toast.key, None)));
        cx.spawn(async move |this, cx| {
            let images = cx
                .background_executor()
                .spawn(async move {
                    read_artwork(&pending)
                        .map(|images| {
                            images
                                .filter_map(|(key, bytes)| Some((key, render_artwork(&bytes)?)))
                                .collect::<Vec<_>>()
                        })
                        .ok_or(pending)
                })
                .await;
            let _ = this.update(cx, |notch, cx| match images {
                Ok(images) => {
                    for (key, image) in images {
                        if let Some(slot) = notch.notifications.artwork.get_mut(&key) {
                            *slot = Some(image);
                            cx.notify();
                        }
                    }
                }
                Err(pending) => {
                    for toast in pending {
                        notch.notifications.artwork.remove(&toast.key);
                    }
                }
            });
        })
        .detach();
    }

    fn sync_notification_icons(
        &mut self,
        apps: std::collections::HashMap<
            String,
            Option<::windows::ApplicationModel::AppDisplayInfo>,
        >,
        cx: &mut Context<Self>,
    ) {
        self.notifications.icons.retain(|id, icon| {
            if apps.contains_key(id) {
                return true;
            }
            if let Some(image) = icon.take() {
                cx.drop_image(image, None);
            }
            false
        });

        for (id, app) in apps {
            if self.notifications.icons.contains_key(&id) {
                continue;
            }
            self.notifications.icons.insert(id.clone(), None);
            cx.spawn(async move |this, cx| {
                let logo = match app {
                    Some(app) => read_icon(app).await,
                    None => None,
                };
                let (id, icon) = cx
                    .background_executor()
                    .spawn(async move {
                        let icon = logo
                            .as_deref()
                            .and_then(render_icon)
                            .or_else(|| render_icon(&read_registered_icon(&id)?))
                            .or_else(|| render_icon_pixels(&read_shell_icon(&id)?));
                        (id, icon)
                    })
                    .await;
                let _ = this.update(cx, |notch, cx| {
                    if let Some(entry) = notch.notifications.icons.get_mut(&id) {
                        *entry = icon;
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    fn sync_placement(&self, window: &Window, cx: &App) {
        let settings = self.appearance.read(cx);
        if let Some(monitor) = selected_monitor(&self.displays, settings.monitor.as_deref())
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
        let hovered = window.is_window_hovered();
        if self.hovered != hovered {
            self.hovered = hovered;
            match self.mode {
                Mode::Compact | Mode::Collapsed if hovered => {
                    self.set_mode(Mode::Music, cx);
                }
                Mode::Music | Mode::Notifications if !hovered => {
                    self.set_mode(self.hover_mode(), cx);
                }
                _ => {}
            }
            self.update_idle_start(cx);
        }

        let settings = self.appearance.read(cx);
        let background = settings.background.color();
        let rounded = settings.corners == Corners::Rounded;
        let screen_height = window.display(cx).map_or(f32::INFINITY, |display| {
            f32::from(display.bounds().size.height)
        });
        let target = match self.mode {
            Mode::Compact => self.compact_size,
            Mode::Collapsed => COLLAPSED_SIZE,
            Mode::Music => EXPANDED_SIZE,
            Mode::Notifications => (
                EXPANDED_SIZE.0,
                self.compact_size.1 + self.notifications.list_height(),
            ),
            Mode::Preview(_) => (EXPANDED_SIZE.0, PREVIEW_HEIGHT),
        };
        let target = (target.0, target.1.min(screen_height));
        if target != self.target_size {
            self.target_size = target;
            self.animation = Some(SizeAnimation::new((self.width, self.height), target));
        }
        if let Some(animation) = &self.animation {
            let (width, height, finished) = animation.sample();
            let (min_width, min_height) = animation.min_size();
            self.width = width.max(self.compact_size.0.min(min_width));
            let min_height = self.compact_size.1.min(min_height).min(screen_height);
            self.height = height.clamp(min_height, screen_height);
            self.sync_placement(window, cx);

            if finished {
                self.animation = None;
            }
        }

        let expansion = ((self.width - self.compact_size.0)
            / (EXPANDED_SIZE.0 - self.compact_size.0))
            .clamp(0.0, 1.0);

        // Let the date reveal gradually even when the window width grows quickly.
        let date_limit = ((expansion - 0.25) / 0.70).clamp(0.0, 1.0);
        if target.0 > self.compact_size.0 && self.date_reveal < date_limit {
            let now = Instant::now();
            let previous_frame = self.date_reveal_frame.replace(now).unwrap_or(now);
            let progress = (now - previous_frame).as_secs_f32() / DATE_REVEAL_SECONDS;
            self.date_reveal = (self.date_reveal + progress).min(date_limit);
        } else {
            self.date_reveal_frame = None;
            self.date_reveal = self.date_reveal.min(date_limit);
        }
        if self.animation.is_some() || self.date_reveal_frame.is_some() {
            window.request_animation_frame();
        }
        let date_opacity = self.date_reveal;
        let player_opacity = ((expansion - 0.6) / 0.35).clamp(0.0, 1.0);

        let notch = div()
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
            .font_weight(FontWeight(600.0));

        if self.mode == Mode::Collapsed {
            return notch;
        }

        if let Mode::Preview(key) = self.mode
            && let Some(toast) = self.notifications.find(key)
        {
            let view = cx.weak_entity();
            return notch.child(
                self.notifications
                    .preview(toast, move |key, cx| {
                        let _ = view.update(cx, |notch, cx| notch.dismiss_notification(key, cx));
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_notifications(cx))),
            );
        }

        notch
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
            .when(self.mode == Mode::Music && player_opacity > 0.0, |notch| {
                notch.child(
                    div()
                        .opacity(player_opacity)
                        .child(self.music_player.clone()),
                )
            })
            .when(self.mode == Mode::Notifications, |notch| {
                let view = cx.weak_entity();
                let dismiss_view = view.clone();
                let height = self.height.min(target.1) - self.compact_size.1;
                notch.child(self.notifications.panel(
                    height,
                    window,
                    cx.listener(move |this, event: &ScrollWheelEvent, window, cx| {
                        if this
                            .notifications
                            .scroll(height, event.delta, window.line_height())
                        {
                            cx.notify();
                        }
                    }),
                    cx.listener(|this, _, _, cx| this.request_notification_access(cx)),
                    move |key, cx| {
                        let _ = view.update(cx, |this, cx| this.open_notification_app(key, cx));
                    },
                    move |key, cx| {
                        let _ = dismiss_view
                            .update(cx, |notch, cx| notch.dismiss_notification(key, cx));
                    },
                ))
            })
            .when(
                matches!(self.mode, Mode::Music | Mode::Notifications) && player_opacity > 0.0,
                |notch| {
                    notch
                        .child(
                            self.notifications.bell().opacity(player_opacity).on_click(
                                cx.listener(|this, _, _, cx| this.toggle_notifications(cx)),
                            ),
                        )
                        .child(
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
                },
            )
    }
}
