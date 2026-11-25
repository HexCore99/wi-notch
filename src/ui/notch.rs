mod playback_wave;
mod pomodoro;

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use super::animation::{PageDirection, SizeAnimation, SizeTransition, page_transition_frame};
use super::calendar::{self, Calendar};
use super::music_player::MusicPlayer;
use super::notifications::{
    Notifications, PREVIEW_HEIGHT, render_artwork, render_icon, render_icon_pixels,
};
use super::prayer_times;
use super::settings::{Page as SettingsPage, SettingsView};
use super::volume::{VOLUME_HUD_SIZE, VolumeFeedback, volume_bar};
use crate::appearance::{AppearanceSettings, Corners};
use crate::notch_geometry::{COLLAPSED_SIZE, EXPANDED_SIZE};
use crate::prayer::{PrayerReminder, PrayerReminders, Schedule, city};
use crate::ui::clock::Clock;
use crate::windows::app_icons::{read_registered_icon, read_shell_icon};
use crate::windows::audio::{VolumeKeyWatch, play_system_sound, read_default_output};
use crate::windows::display::{Monitor, monitors, selected_monitor};
use crate::windows::media::SessionManager;
use crate::windows::notifications::{
    Access, Listener, Snapshot, Toast, ToastKey, open_notification, read_artwork, read_icon,
};
use chrono::{DateTime, Local, NaiveDate, Utc};
use gpui::{prelude::*, *};
use playback_wave::{PEAK_INTERVAL, ReactiveWave, WaveFrame};
use pomodoro::Pomodoro;

const DATE_REVEAL_SECONDS: f32 = 0.38;
const PAGE_ARROW_WIDTH: f32 = 24.0;
const PAGE_ARROW_HEIGHT: f32 = 26.0;
pub(super) const PAGE_BODY_WIDTH: f32 = EXPANDED_SIZE.0 - PAGE_ARROW_WIDTH * 2.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Music,
    PrayerTimes,
    Pomodoro,
}

impl Page {
    fn neighbor(self, direction: PageDirection) -> Self {
        match (self, direction) {
            (Self::Music, PageDirection::Right) | (Self::Pomodoro, PageDirection::Left) => {
                Self::PrayerTimes
            }
            (Self::PrayerTimes, PageDirection::Right) | (Self::Music, PageDirection::Left) => {
                Self::Pomodoro
            }
            (Self::Pomodoro, PageDirection::Right) | (Self::PrayerTimes, PageDirection::Left) => {
                Self::Music
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Compact,
    Collapsed,
    Expanded(Page),
    Notifications,
    Preview(ToastKey),
}

pub struct Notch {
    pub width: f32,
    pub height: f32,

    compact_size: (f32, f32),
    hovered: bool,
    playback_started: Option<Instant>,
    reactive_wave: ReactiveWave,
    always_minimal: bool,
    idle_since: Option<Instant>,
    mode: Mode,
    page: Page,
    target_size: (f32, f32),
    animation: Option<SizeAnimation>,
    page_switch: Option<(Instant, PageDirection, Page)>,
    date_reveal: f32,
    date_reveal_frame: Option<Instant>,
    volume_stretch: f32,
    displays: Vec<Monitor>,
    _placement_watch: Task<()>,
    _playback_watch: Task<()>,
    _volume_watch: Task<()>,
    _notification_watch: Option<Task<()>>,
    preview_timeout: Option<Task<()>>,
    opening_notification: bool,

    clock: Entity<Clock>,
    calendar: Option<Calendar>,
    music_player: Entity<MusicPlayer>,
    prayer_schedule: Option<Schedule>,
    prayer_refresh: Option<Task<()>>,
    prayer_reminders: PrayerReminders,
    prayer_banner: Option<(PrayerReminder, Instant)>,
    pomodoro: Pomodoro,
    volume_feedback: VolumeFeedback,
    volume_keys: Option<VolumeKeyWatch>,
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
        cx.observe_in(&appearance, window, |this, appearance, window, cx| {
            let settings = appearance.read(cx);
            this.pomodoro
                .clear_removed_session(&settings.pomodoro_session_names);
            if !settings.sync_visualizer {
                this.reactive_wave = ReactiveWave::default();
            }
            this.sync_prayer_settings(cx);
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
                        if notch.playback_started.is_some() != playing {
                            notch.playback_started = playing.then(Instant::now);
                            notch.reactive_wave = ReactiveWave::default();
                            cx.notify();
                        }
                        notch.sync_idle_policy(cx);
                    })
                    .is_err()
                {
                    break;
                }
                cx.background_executor().timer(Duration::from_secs(1)).await;
            }
        });
        let volume_watch = cx.spawn(async move |this, cx| {
            let executor = cx.background_executor().clone();
            loop {
                let Ok(read_peak) = this.read_with(cx, |notch, cx| notch.should_read_peak(cx))
                else {
                    break;
                };
                let reading = executor
                    .spawn(async move { read_default_output(read_peak) })
                    .await;
                if this
                    .update(cx, |notch, cx| {
                        let now = Instant::now();
                        let wave_changed = read_peak
                            && notch.should_read_peak(cx)
                            && notch
                                .reactive_wave
                                .push_peak(reading.as_ref().and_then(|output| output.peak), now);
                        let presses = notch
                            .volume_keys
                            .as_ref()
                            .map(VolumeKeyWatch::take_presses)
                            .unwrap_or_default();
                        let volume_changed = notch.volume_feedback.update(reading, now, presses);
                        if wave_changed || volume_changed {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
                let interval = if read_peak {
                    PEAK_INTERVAL
                } else {
                    Duration::from_millis(125)
                };
                executor.timer(interval).await;
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
        let settings = appearance.read(cx);
        let always_minimal = settings.always_minimal;
        let prayer_reminders = PrayerReminders::new(
            settings.prayer_city_id,
            settings.prayer_reminders,
            Utc::now(),
        );
        let compact_size = (width, height);
        let (mode, (width, height)) = if always_minimal {
            (Mode::Collapsed, COLLAPSED_SIZE)
        } else {
            (Mode::Compact, compact_size)
        };
        let clock = cx.new(Clock::new);
        cx.observe(&clock, |this, _, cx| {
            if this.calendar.is_some() {
                cx.notify();
            }
        })
        .detach();
        let mut notch = Self {
            width,
            height,
            compact_size,
            hovered: false,
            playback_started: None,
            reactive_wave: ReactiveWave::default(),
            always_minimal,
            idle_since: None,
            mode,
            page: Page::Music,
            target_size: (width, height),
            animation: None,
            page_switch: None,
            date_reveal: 0.0,
            date_reveal_frame: None,
            volume_stretch: 0.0,
            displays: monitors(),
            _placement_watch: placement_watch,
            _playback_watch: playback_watch,
            _volume_watch: volume_watch,
            _notification_watch: notification_watch,
            preview_timeout: None,
            opening_notification: false,

            clock,
            calendar: None,
            music_player: cx.new(|_| MusicPlayer::default()),
            prayer_schedule: None,
            prayer_refresh: None,
            prayer_reminders,
            prayer_banner: None,
            pomodoro: Pomodoro::default(),
            volume_feedback: VolumeFeedback::default(),
            volume_keys: VolumeKeyWatch::start()
                .inspect_err(|error| eprintln!("Failed to watch volume keys: {error}"))
                .ok(),
            notifications: Notifications::new(access),
            appearance,
            settings_window: None,
        };
        notch.update_idle_start(cx);
        notch.sync_prayer_refresh(cx);
        notch.sync_placement(window, cx);
        notch
    }

    fn hover_mode(&self) -> Mode {
        if self.hovered {
            Mode::Expanded(self.page)
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
        let music_visible = mode == Mode::Expanded(Page::Music);
        if (self.mode == Mode::Expanded(Page::Music)) != music_visible {
            self.music_player
                .update(cx, |player, cx| player.set_visible(music_visible, cx));
        }
        self.mode = mode;
        if mode != Mode::Compact {
            self.reactive_wave = ReactiveWave::default();
        }
        self.calendar = None;
        self.sync_prayer_refresh(cx);
        if mode != Mode::Expanded(Page::Pomodoro) {
            self.close_pomodoro_menu(cx);
        }
        if !matches!(mode, Mode::Expanded(_)) {
            self.page_switch = None;
        }
        if !matches!(mode, Mode::Preview(_)) {
            self.preview_timeout = None;
        }
        self.update_idle_start(cx);
        cx.notify();
    }

    fn sync_prayer_settings(&mut self, cx: &mut Context<Self>) {
        let settings = self.appearance.read(cx);
        if self.prayer_reminders.configure(
            settings.prayer_city_id,
            settings.prayer_reminders,
            Utc::now(),
        ) {
            self.prayer_banner = None;
            self.sync_prayer_refresh(cx);
        }
    }

    fn sync_prayer_refresh(&mut self, cx: &mut Context<Self>) {
        let needed = self.mode == Mode::Expanded(Page::PrayerTimes)
            || self.appearance.read(cx).prayer_reminders.enabled();
        if needed == self.prayer_refresh.is_some() {
            return;
        }
        self.prayer_refresh = needed.then(|| {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    if this
                        .update(cx, |notch, cx| notch.refresh_prayers(Utc::now(), cx))
                        .is_err()
                    {
                        break;
                    }
                }
            })
        });
    }

    fn update_prayer_schedule(&mut self, city_id: u32, now: DateTime<Utc>) {
        let selected = city(city_id);
        if !self
            .prayer_schedule
            .as_ref()
            .is_some_and(|schedule| schedule.is_current(selected.id, now))
        {
            self.prayer_schedule = Some(Schedule::new(selected, now));
        }
    }

    fn refresh_prayers(&mut self, now: DateTime<Utc>, cx: &mut Context<Self>) {
        let instant = Instant::now();
        let expired = self
            .prayer_banner
            .is_some_and(|(_, until)| instant >= until);
        if expired {
            self.prayer_banner = None;
        }

        let settings = self.appearance.read(cx);
        if settings.prayer_reminders.enabled() {
            self.update_prayer_schedule(settings.prayer_city_id, now);
            if let Some(schedule) = &self.prayer_schedule
                && let Some(reminder) = self.prayer_reminders.check(schedule, now)
            {
                play_system_sound();
                self.prayer_banner = Some((reminder, instant + Duration::from_secs(4)));
                cx.notify();
                return;
            }
        }

        if expired || self.mode == Mode::Expanded(Page::PrayerTimes) {
            cx.notify();
        }
    }

    fn should_read_peak(&self, cx: &App) -> bool {
        self.mode == Mode::Compact
            && self.playback_started.is_some()
            && self.appearance.read(cx).sync_visualizer
    }

    fn update_idle_start(&mut self, cx: &App) {
        if self.mode == Mode::Compact
            && !self.hovered
            && self.playback_started.is_none()
            && !self.pomodoro.in_cycle()
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

        if self.mode == Mode::Collapsed
            && (was_always_minimal
                || !auto_minimal
                || self.playback_started.is_some()
                || self.pomodoro.in_cycle())
        {
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

    fn toggle_calendar(&mut self, cx: &mut Context<Self>) {
        if self.calendar.take().is_some() {
            if self.mode == Mode::Notifications {
                self.notifications.mark_read();
            }
        } else {
            self.calendar = Some(Calendar::default());
        }
        cx.notify();
    }

    pub(super) fn show_calendar_month(
        &mut self,
        month: NaiveDate,
        today: NaiveDate,
        cx: &mut Context<Self>,
    ) {
        if let Some(calendar) = &mut self.calendar {
            calendar.show_month(month, today);
            cx.notify();
        }
    }

    fn date_label(&self, today: NaiveDate, cx: &Context<Self>) -> Stateful<Div> {
        div()
            .id("notch-date")
            .child(today.format("%a %d %b %Y").to_string())
            .when(
                self.page_switch.is_none()
                    && matches!(self.mode, Mode::Expanded(_) | Mode::Notifications),
                |date| {
                    date.cursor_pointer()
                        .hover(|style| style.text_color(rgb(0xba9cff)))
                        .on_click(cx.listener(|notch, _, _, cx| notch.toggle_calendar(cx)))
                },
            )
    }

    fn switch_page(&mut self, direction: PageDirection, cx: &mut Context<Self>) {
        if self.page_switch.is_some() || !matches!(self.mode, Mode::Expanded(_)) {
            return;
        }
        let previous = self.page;
        self.page = self.page.neighbor(direction);
        self.page_switch = Some((Instant::now(), direction, previous));
        self.set_mode(Mode::Expanded(self.page), cx);
    }

    fn page_arrow(
        &self,
        direction: PageDirection,
        opacity: f32,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let left = matches!(direction, PageDirection::Left);
        let (id, icon) = if left {
            ("previous-page", "‹")
        } else {
            ("next-page", "›")
        };
        div()
            .id(id)
            .absolute()
            .top(px(((self.height - PAGE_ARROW_HEIGHT) / 2.0).max(0.0)))
            .when(left, |arrow| arrow.left(px(0.0)))
            .when(!left, |arrow| arrow.right(px(0.0)))
            .w(px(PAGE_ARROW_WIDTH))
            .h(px(PAGE_ARROW_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .text_size(px(20.0))
            .text_color(rgb(0xb6b4bf))
            .opacity(opacity)
            .cursor_pointer()
            .hover(|arrow| arrow.bg(rgba(0xffffff20)).text_color(rgb(0xffffff)))
            .child(icon)
            .on_click(cx.listener(move |this, _, _, cx| this.switch_page(direction, cx)))
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
            let (changed, newest) = self.notifications.sync(
                snapshot.toasts,
                self.mode == Mode::Notifications && self.calendar.is_none(),
            );
            self.sync_notification_icons(snapshot.apps, cx);

            if let Some(key) = newest
                && !self.always_minimal
                && self.prayer_banner.is_none()
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
                self.width + self.volume_stretch.abs(),
                self.height,
                self.volume_stretch / 2.0,
                settings.top_gap,
                monitor,
            )
        {
            eprintln!("Failed to position the notch: {error}");
        }
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        let page = match self.mode {
            Mode::Expanded(Page::PrayerTimes) => Some(SettingsPage::PrayerTimes),
            Mode::Expanded(Page::Pomodoro) => Some(SettingsPage::Pomodoro),
            _ => None,
        };
        self.open_settings_page(page, cx);
    }

    fn open_settings_page(&mut self, page: Option<SettingsPage>, cx: &mut Context<Self>) {
        if self.calendar.is_some() {
            self.toggle_calendar(cx);
        }
        if self.page_switch.take().is_some() {
            cx.notify();
        }
        if let Some(handle) = self.settings_window
            && handle
                .update(cx, |view, window, cx| {
                    if let Some(page) = page {
                        view.show_page(page, cx);
                    }
                    window.activate_window();
                })
                .is_ok()
        {
            return;
        }

        self.settings_window = match SettingsView::open(
            self.appearance.clone(),
            page.unwrap_or(SettingsPage::General),
            cx,
        ) {
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
        let resting_size = match self.mode {
            Mode::Compact => Some(self.compact_size),
            Mode::Collapsed => Some(COLLAPSED_SIZE),
            _ => None,
        };
        let hovered = window.is_window_hovered()
            && resting_size.is_none_or(|(width, height)| {
                let viewport = window.viewport_size();
                if viewport.width <= px(width) && viewport.height <= px(height) {
                    return true;
                }
                // A wider volume bar should only open the player over the resting notch.
                let inset =
                    ((f32::from(viewport.width) - width - self.volume_stretch) / 2.0).max(0.0);
                let bounds = Bounds::new(point(px(inset), px(0.0)), size(px(width), px(height)));
                crate::windows::window::cursor_in_client_bounds(window, bounds)
            });
        if self.hovered != hovered {
            self.hovered = hovered;
            match self.mode {
                Mode::Compact | Mode::Collapsed if hovered => {
                    self.set_mode(self.hover_mode(), cx);
                }
                Mode::Expanded(_) | Mode::Notifications if !hovered => {
                    self.set_mode(self.hover_mode(), cx);
                }
                _ => {}
            }
            self.update_idle_start(cx);
        }

        let now = Instant::now();
        let page_transition = self.page_switch.and_then(|(started, direction, previous)| {
            page_transition_frame(now.saturating_duration_since(started), direction)
                .map(|frame| (frame, previous))
        });
        if page_transition.is_none() {
            self.page_switch = None;
        }
        let visible_page = match (self.mode, &page_transition) {
            (Mode::Expanded(_), Some((frame, previous))) if !frame.incoming => Some(*previous),
            (Mode::Expanded(page), _) => Some(page),
            _ => None,
        };
        let page_opacity = page_transition
            .as_ref()
            .map_or(1.0, |(frame, _)| frame.opacity);
        let page_offset = px(page_transition
            .as_ref()
            .map_or(0.0, |(frame, _)| frame.offset_x));

        let settings = self.appearance.read(cx);
        let background = settings.background.color();
        let rounded = settings.corners == Corners::Rounded;
        let sync_visualizer = settings.sync_visualizer;
        let prayer_visible = visible_page == Some(Page::PrayerTimes);
        let prayer = if prayer_visible || self.mode == Mode::Expanded(Page::PrayerTimes) {
            let now = Utc::now();
            self.update_prayer_schedule(settings.prayer_city_id, now);
            self.prayer_schedule
                .as_ref()
                .filter(|_| prayer_visible)
                .map(|schedule| (schedule, now))
        } else {
            None
        };
        let today = prayer.map_or_else(
            || Local::now().date_naive(),
            |(schedule, now)| now.with_timezone(&schedule.city.zone).date_naive(),
        );
        let calendar_open = self.calendar.is_some();
        let screen_height = window.display(cx).map_or(f32::INFINITY, |display| {
            f32::from(display.bounds().size.height)
        });
        let volume = self.volume_feedback.frame(Instant::now());
        let standalone_reminder = self.prayer_banner.filter(|_| {
            matches!(
                self.mode,
                Mode::Compact | Mode::Collapsed | Mode::Preview(_)
            )
        });
        let volume_hud = volume.filter(|_| {
            standalone_reminder.is_none() && matches!(self.mode, Mode::Compact | Mode::Collapsed)
        });
        let target = if calendar_open {
            (
                EXPANDED_SIZE.0,
                self.compact_size.1 + calendar::PANEL_HEIGHT,
            )
        } else if standalone_reminder.is_some() {
            (EXPANDED_SIZE.0, PREVIEW_HEIGHT)
        } else if volume_hud.is_some() {
            VOLUME_HUD_SIZE
        } else {
            match self.mode {
                Mode::Compact => self.compact_size,
                Mode::Collapsed => COLLAPSED_SIZE,
                Mode::Expanded(_) => EXPANDED_SIZE,
                Mode::Notifications => (
                    EXPANDED_SIZE.0,
                    self.compact_size.1 + self.notifications.list_height(),
                ),
                Mode::Preview(_) => (EXPANDED_SIZE.0, PREVIEW_HEIGHT),
            }
        };
        let target = (target.0, target.1.min(screen_height));
        if target != self.target_size {
            let previous_height = self.target_size.1;
            let transition = if volume_hud.is_some() {
                SizeTransition::Resize
            } else if target.1 > previous_height {
                SizeTransition::Expand
            } else if target.1 < previous_height && previous_height > VOLUME_HUD_SIZE.1 {
                SizeTransition::Shrink
            } else {
                SizeTransition::Resize
            };
            self.target_size = target;
            self.animation = Some(SizeAnimation::new(
                (self.width, self.height),
                target,
                transition,
            ));
        }
        let resizing = self.animation.is_some();
        if let Some(animation) = &self.animation {
            let (width, height, finished) = animation.sample();
            let (min_width, min_height) = animation.min_size();
            self.width = width.max(self.compact_size.0.min(min_width));
            let min_height = self.compact_size.1.min(min_height).min(screen_height);
            self.height = height.clamp(min_height, screen_height);
            if finished {
                self.animation = None;
            }
        }
        let volume_stretch = volume_hud.map_or(0.0, |frame| frame.stretch);
        if resizing || self.volume_stretch != volume_stretch {
            self.volume_stretch = volume_stretch;
            self.sync_placement(window, cx);
        }
        let width = self.width + self.volume_stretch.abs();

        let expansion = ((self.width - self.compact_size.0)
            / (EXPANDED_SIZE.0 - self.compact_size.0))
            .clamp(0.0, 1.0);

        // Let the date reveal gradually even when the window width grows quickly.
        let date_limit = if volume_hud.is_some() {
            0.0
        } else {
            ((expansion - 0.25) / 0.70).clamp(0.0, 1.0)
        };
        if target.0 > self.compact_size.0 && self.date_reveal < date_limit {
            let now = Instant::now();
            let previous_frame = self.date_reveal_frame.replace(now).unwrap_or(now);
            let progress = (now - previous_frame).as_secs_f32() / DATE_REVEAL_SECONDS;
            self.date_reveal = (self.date_reveal + progress).min(date_limit);
        } else {
            self.date_reveal_frame = None;
            self.date_reveal = self.date_reveal.min(date_limit);
        }
        let date_opacity = self.date_reveal;
        let wave = self
            .playback_started
            .filter(|_| {
                self.mode == Mode::Compact
                    && !calendar_open
                    && standalone_reminder.is_none()
                    && volume_hud.is_none()
                    && date_opacity == 0.0
                    && self.width >= self.compact_size.0 - 1.0
            })
            .and_then(|started| {
                if sync_visualizer {
                    self.reactive_wave.frame(now)
                } else {
                    Some(WaveFrame::looping(now.saturating_duration_since(started)))
                }
            });
        if self.animation.is_some()
            || self.date_reveal_frame.is_some()
            || page_transition.is_some()
            || volume.is_some_and(|frame| frame.animating)
            || wave.as_ref().is_some_and(|frame| frame.animating)
        {
            window.request_animation_frame();
        }
        let player_opacity = ((expansion - 0.6) / 0.35).clamp(0.0, 1.0);
        let content_opacity = player_opacity * page_opacity;
        let page_body_visible = !calendar_open && content_opacity > 0.0;

        let notch = div()
            .id("notch")
            .w(px(width))
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

        if let Some((reminder, _)) = standalone_reminder {
            return notch
                .justify_center()
                .child(prayer_times::reminder_banner(reminder).opacity(player_opacity));
        }

        if let Some(volume) = volume_hud {
            let opacity = ((self.height - COLLAPSED_SIZE.1) / 22.0).clamp(0.0, 1.0);
            return notch.justify_center().child(
                volume_bar(volume)
                    .w(px((width - 30.0).max(0.0)))
                    .opacity(opacity),
            );
        }

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

        let header = div()
            .h(px(self.compact_size.1))
            .flex_none()
            .flex()
            .items_center();
        let header = if let Some((reminder, _)) = self.prayer_banner {
            header.child(prayer_times::reminder_banner(reminder))
        } else if let Some(volume) = volume {
            let available_width = (self.width - 88.0).clamp(0.0, 226.0);
            header.child(
                volume_bar(volume)
                    .w(px(available_width + volume.stretch.abs()))
                    .relative()
                    .left(px(volume.stretch / 2.0)),
            )
        } else {
            let header = header.when(page_transition.is_some(), |header| {
                header.relative().left(page_offset).opacity(page_opacity)
            });
            if let Some((schedule, now)) = prayer {
                header.child(
                    prayer_times::header(schedule, now, self.date_label(today, cx))
                        .opacity(player_opacity),
                )
            } else if self.mode == Mode::Compact && self.pomodoro.in_cycle() {
                header
                    .relative()
                    .w_full()
                    .justify_center()
                    .child(self.clock.clone())
                    .child(
                        div()
                            .absolute()
                            .left(px(10.0))
                            .top(px(0.0))
                            .bottom(px(0.0))
                            .flex()
                            .items_center()
                            .child(self.compact_pomodoro(now)),
                    )
            } else {
                header
                    .gap(px(8.0 * date_opacity))
                    .when(date_opacity > 0.0, |header| {
                        header.child(
                            self.date_label(today, cx)
                                .w(px(104.0 * date_opacity))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .opacity(date_opacity)
                                .text_size(px(12.0)),
                        )
                    })
                    .child(self.clock.clone())
            }
        };

        let header = header.when_some(wave, |header, frame| {
            header.relative().w_full().justify_center().child(
                frame
                    .bars()
                    .absolute()
                    .right(px(10.0))
                    .top(px(0.0))
                    .bottom(px(0.0)),
            )
        });

        notch
            .child(header)
            .when_some(self.calendar, |notch, calendar| {
                notch.child(calendar.panel(today, cx).opacity(player_opacity))
            })
            .when(
                visible_page == Some(Page::Music) && page_body_visible,
                |notch| {
                    notch.child(
                        div()
                            .relative()
                            .left(page_offset)
                            .opacity(content_opacity)
                            .child(self.music_player.clone()),
                    )
                },
            )
            .when_some(
                prayer.filter(|_| page_body_visible),
                |notch, (schedule, now)| {
                    notch.child(
                        prayer_times::panel(schedule, now, background)
                            .relative()
                            .left(page_offset)
                            .opacity(content_opacity),
                    )
                },
            )
            .when(
                visible_page == Some(Page::Pomodoro) && page_body_visible,
                |notch| {
                    notch.child(
                        self.pomodoro_panel(now, cx)
                            .relative()
                            .left(page_offset)
                            .opacity(content_opacity),
                    )
                },
            )
            .when(page_transition.is_some(), |notch| {
                notch.child(
                    div()
                        .absolute()
                        .top(px(self.compact_size.1))
                        .bottom(px(0.0))
                        .left(px(PAGE_ARROW_WIDTH))
                        .right(px(PAGE_ARROW_WIDTH))
                        .occlude(),
                )
            })
            .when(
                !calendar_open && matches!(self.mode, Mode::Expanded(_)) && player_opacity > 0.0,
                |notch| {
                    notch
                        .child(self.page_arrow(PageDirection::Left, player_opacity, cx))
                        .child(self.page_arrow(PageDirection::Right, player_opacity, cx))
                },
            )
            .when(
                !calendar_open && self.mode == Mode::Notifications,
                |notch| {
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
                },
            )
            .when(
                matches!(self.mode, Mode::Expanded(_) | Mode::Notifications)
                    && player_opacity > 0.0,
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
