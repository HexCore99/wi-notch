use std::time::Duration;

use super::animation::SizeAnimation;
use super::music_player::MusicPlayer;
use super::notifications::{Notifications, PREVIEW_HEIGHT, render_icon};
use super::settings::SettingsView;
use crate::appearance::{AppearanceSettings, Corners};
use crate::notch_geometry::EXPANDED_SIZE;
use crate::ui::clock::Clock;
use crate::windows::display::{monitors, selected_monitor};
use crate::windows::notifications::{
    Access, Listener, Snapshot, ToastKey, open_notification, read_icon,
};
use chrono::Local;
use gpui::{prelude::*, *};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Compact,
    Music,
    Notifications,
    Preview(ToastKey),
}

pub struct Notch {
    pub width: f32,
    pub height: f32,

    compact_size: (f32, f32),
    hovered: bool,
    mode: Mode,
    target_size: (f32, f32),
    animation: Option<SizeAnimation>,
    _placement_watch: Task<()>,
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
        cx.observe(&appearance, |_, _, cx| cx.notify()).detach();
        cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() && this.mode == Mode::Notifications {
                this.set_mode(Mode::Compact, cx);
            }
        })
        .detach();
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
        let listener = Listener::connect();
        let access = listener
            .as_ref()
            .map_or_else(|access| *access, Listener::access);
        let notification_watch = listener.ok().map(|listener| {
            cx.spawn_in(window, async move |this, cx| {
                loop {
                    let mut access = listener.access();
                    let snapshot = if access == Access::Allowed {
                        let snapshot = listener.read().await.ok();
                        // Discard a read if access was revoked while waiting for Windows.
                        access = listener.access();
                        snapshot.filter(|_| access == Access::Allowed)
                    } else {
                        None
                    };
                    if this
                        .update_in(cx, |notch, _, cx| {
                            notch.sync_notifications(access, snapshot, cx)
                        })
                        .is_err()
                    {
                        break;
                    }
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                }
            })
        });
        Self {
            width,
            height,
            compact_size: (width, height),
            hovered: false,
            mode: Mode::Compact,
            target_size: (width, height),
            animation: None,
            _placement_watch: placement_watch,
            _notification_watch: notification_watch,
            preview_timeout: None,
            opening_notification: false,

            clock: cx.new(Clock::new),
            music_player: cx.new(|_| MusicPlayer::default()),
            notifications: Notifications::new(access),
            appearance,
            settings_window: None,
        }
    }

    fn hover_mode(&self) -> Mode {
        if self.hovered {
            Mode::Music
        } else {
            Mode::Compact
        }
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
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
        cx.notify();
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
                        this.notifications.launch_error = Some(key);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
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
            let (changed, newest) = self
                .notifications
                .sync(snapshot.toasts, self.mode == Mode::Notifications);
            self.sync_notification_icons(snapshot.apps, cx);

            if let Some(key) = newest
                && matches!(self.mode, Mode::Compact | Mode::Preview(_))
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

    fn sync_notification_icons(
        &mut self,
        apps: std::collections::HashMap<String, ::windows::ApplicationModel::AppDisplayInfo>,
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
                let Some(bytes) = read_icon(app).await else {
                    return;
                };
                let icon = cx
                    .background_executor()
                    .spawn(async move { render_icon(&bytes) })
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
        let hovered = window.is_window_hovered();
        if self.hovered != hovered {
            self.hovered = hovered;
            match self.mode {
                Mode::Compact if hovered => self.set_mode(Mode::Music, cx),
                Mode::Music | Mode::Notifications if !hovered => self.set_mode(Mode::Compact, cx),
                _ => {}
            }
        }

        let settings = self.appearance.read(cx);
        let background = settings.background.color();
        let rounded = settings.corners == Corners::Rounded;
        let screen_height = window.display(cx).map_or(f32::INFINITY, |display| {
            f32::from(display.bounds().size.height)
        });
        let target = match self.mode {
            Mode::Compact => self.compact_size,
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
            self.width = width.max(self.compact_size.0);
            self.height = height.clamp(self.compact_size.1.min(screen_height), screen_height);

            if finished {
                self.animation = None;
            } else {
                window.request_animation_frame();
            }
        }

        self.sync_placement(window, cx);

        let expansion = ((self.width - self.compact_size.0)
            / (EXPANDED_SIZE.0 - self.compact_size.0))
            .clamp(0.0, 1.0);

        let date_opacity = ((expansion - 0.25) / 0.55).clamp(0.0, 1.0);
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

        if let Mode::Preview(key) = self.mode
            && let Some(toast) = self.notifications.find(key)
        {
            return notch.child(
                self.notifications
                    .preview(toast)
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
                notch.child(self.notifications.panel(
                    self.height.min(target.1) - self.compact_size.1,
                    cx.listener(|this, _, _, cx| this.request_notification_access(cx)),
                    move |key, cx| {
                        let _ = view.update(cx, |this, cx| this.open_notification_app(key, cx));
                    },
                ))
            })
            .child(
                self.notifications
                    .bell()
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_notifications(cx))),
            )
            .when(
                matches!(self.mode, Mode::Music | Mode::Notifications) && player_opacity > 0.0,
                |notch| {
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
                },
            )
    }
}
