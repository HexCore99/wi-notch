use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    sync::Arc,
    time::{Duration, Instant},
};

use ::image::{
    DynamicImage, Frame, ImageReader, Limits, RgbaImage,
    imageops::{FilterType, crop_imm},
};
use gpui::{prelude::*, *};

use crate::windows::notifications::{Access, Toast, ToastKey};

const ROW_HEIGHT: f32 = 70.0;
const ROW_GAP: f32 = 6.0;
const ROW_STRIDE: f32 = ROW_HEIGHT + ROW_GAP;
const LIST_PADDING: f32 = 4.0;
const MAX_ROWS: usize = 6;
const SCROLL_DURATION: Duration = Duration::from_millis(120);
const ROW_FADE_DURATION: Duration = Duration::from_millis(240);
const ICON_SIZE: u32 = 18;
pub const PREVIEW_HEIGHT: f32 = 64.0;

#[derive(Default)]
struct ScrollAnimation {
    from: f32,
    target: f32,
    started: Option<Instant>,
}

impl ScrollAnimation {
    fn position(&self, now: Instant) -> (f32, bool) {
        let Some(started) = self.started else {
            return (self.target, false);
        };
        let progress =
            (now.duration_since(started).as_secs_f32() / SCROLL_DURATION.as_secs_f32()).min(1.0);
        let eased = 1.0 - (1.0 - progress).powi(2);
        (
            self.from + (self.target - self.from) * eased,
            progress < 1.0,
        )
    }

    fn scroll_by(&mut self, delta: f32, max: f32, precise: bool, now: Instant) -> bool {
        let current = self.position(now).0.clamp(0.0, max);
        let target = self.target.clamp(0.0, max);
        // Reverse immediately if the user spins the wheel the other way.
        let base = if precise || (target - current) * delta < 0.0 {
            current
        } else {
            target
        };
        let next = (base + delta).clamp(0.0, max);
        if next == target && !precise {
            return false;
        }

        self.from = current;
        self.target = next;
        self.started = (!precise && next != current).then_some(now);
        true
    }
}

fn rows_height(count: usize) -> f32 {
    (count as f32 * ROW_STRIDE - ROW_GAP).max(0.0) + 2.0 * LIST_PADDING
}

fn row_opacity(top: f32, viewport_height: f32) -> f32 {
    let visible = ((top + ROW_HEIGHT).min(viewport_height) - top.max(0.0)).max(0.0);
    (visible / ROW_HEIGHT).clamp(0.0, 1.0)
}

fn fade_in_progress(entered: Instant, now: Instant) -> f32 {
    (now.duration_since(entered).as_secs_f32() / ROW_FADE_DURATION.as_secs_f32()).min(1.0)
}

pub struct Notifications {
    pub access: Access,
    pub requesting: bool,
    pub icons: HashMap<String, Option<Arc<RenderImage>>>,
    pub artwork: HashMap<ToastKey, Option<Arc<RenderImage>>>,
    pub action_error: Option<(ToastKey, &'static str)>,
    scroll: ScrollAnimation,
    visible_since: HashMap<ToastKey, Instant>,
    items: Vec<Toast>,
    unread: HashSet<ToastKey>,
    dismissed: HashSet<ToastKey>,
    initialized: bool,
}

impl Notifications {
    pub fn new(access: Access) -> Self {
        Self {
            access,
            requesting: false,
            icons: HashMap::new(),
            artwork: HashMap::new(),
            action_error: None,
            scroll: ScrollAnimation::default(),
            visible_since: HashMap::new(),
            items: Vec::new(),
            unread: HashSet::new(),
            dismissed: HashSet::new(),
            initialized: false,
        }
    }

    pub fn set_access(
        &mut self,
        access: Access,
        mut drop_image: impl FnMut(Arc<RenderImage>),
    ) -> bool {
        if self.access == access {
            return false;
        }
        self.access = access;
        if access != Access::Allowed {
            self.items.clear();
            self.unread.clear();
            self.dismissed.clear();
            self.action_error = None;
            self.initialized = false;
            self.scroll = ScrollAnimation::default();
            self.visible_since.clear();
            for image in self.icons.drain().filter_map(|(_, image)| image) {
                drop_image(image);
            }
            for image in self.artwork.drain().filter_map(|(_, image)| image) {
                drop_image(image);
            }
        }
        true
    }

    pub fn sync(&mut self, mut items: Vec<Toast>, open: bool) -> (bool, Option<ToastKey>) {
        if !self.dismissed.is_empty() {
            // A poll started before the click can still contain the dismissed toast.
            self.dismissed
                .retain(|key| items.iter().any(|toast| toast.key == *key));
            items.retain(|toast| !self.dismissed.contains(&toast.key));
        }
        let changed = !self.initialized || self.items != items;
        if !changed {
            return (false, None);
        }

        let current: HashSet<_> = items.iter().map(|toast| toast.key).collect();
        self.unread.retain(|key| current.contains(key));
        let mut newest = None;
        if self.initialized {
            let previous: HashSet<_> = self.items.iter().map(|toast| toast.key).collect();
            for toast in items.iter().filter(|toast| !previous.contains(&toast.key)) {
                newest.get_or_insert(toast.key);
                if !open {
                    self.unread.insert(toast.key);
                }
            }
        }
        if open && self.scroll.target > 0.0 {
            let offset = self.scroll.position(Instant::now()).0;
            let first = ((offset - LIST_PADDING) / ROW_STRIDE).floor().max(0.0) as usize;
            if let Some(anchor) = self.items.get(first)
                && let Some(index) = items.iter().position(|toast| toast.key == anchor.key)
            {
                // Keep the first visible card in place when the rows above it change.
                let shift = (index as f32 - first as f32) * ROW_STRIDE;
                self.scroll.from = (self.scroll.from + shift).max(0.0);
                self.scroll.target = (self.scroll.target + shift).max(0.0);
            }
        }
        self.initialized = true;
        self.items = items;
        (true, newest)
    }

    pub fn open(&mut self) {
        self.mark_read();
        self.action_error = None;
        self.scroll = ScrollAnimation::default();
        self.visible_since.clear();
    }

    pub fn mark_read(&mut self) {
        self.unread.clear();
    }

    pub fn find(&self, key: ToastKey) -> Option<&Toast> {
        self.items.iter().find(|toast| toast.key == key)
    }

    pub fn dismiss(&mut self, key: ToastKey) {
        self.dismissed.insert(key);
        self.items.retain(|toast| toast.key != key);
        self.unread.remove(&key);
        self.visible_since.remove(&key);
    }

    pub fn list_height(&self) -> f32 {
        if self.items.is_empty() {
            84.0
        } else {
            rows_height(self.items.len().min(MAX_ROWS))
        }
    }

    pub fn scroll(&mut self, height: f32, delta: ScrollDelta, line_height: Pixels) -> bool {
        let pixels = delta.pixel_delta(line_height);
        let distance = -f32::from(if pixels.y == px(0.0) {
            pixels.x
        } else {
            pixels.y
        });
        let max = (rows_height(self.items.len()) - height.max(0.0)).max(0.0);
        distance != 0.0
            && self
                .scroll
                .scroll_by(distance, max, delta.precise(), Instant::now())
    }

    pub fn bell(&self) -> Stateful<Div> {
        div()
            .id("notifications-bell")
            .absolute()
            .top(px(5.0))
            .left(px(8.0))
            .size(px(25.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(rgba(0xffffff20)))
            .text_size(px(14.0))
            .font_family("Segoe UI Symbol")
            .child("🔔\u{fe0e}")
            .when(!self.unread.is_empty(), |bell| {
                bell.child(
                    div()
                        .absolute()
                        .top(px(1.0))
                        .right(px(1.0))
                        .size(px(7.0))
                        .rounded_full()
                        .bg(rgb(0xf54b55)),
                )
            })
    }

    pub fn panel(
        &mut self,
        height: f32,
        window: &Window,
        scroll: impl Fn(&ScrollWheelEvent, &mut Window, &mut App) + 'static,
        enable: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        open_app: impl Fn(ToastKey, &mut App) + Clone + 'static,
        dismiss: impl Fn(ToastKey, &mut App) + Clone + 'static,
    ) -> Stateful<Div> {
        let height = height.max(0.0);
        let panel = div()
            .id("notifications-scroll")
            .w_full()
            .h(px(height))
            .relative()
            .overflow_hidden()
            .px(px(12.0));
        if self.access == Access::Allowed && !self.items.is_empty() {
            let now = Instant::now();
            let (offset, mut animating) = self.scroll.position(now);
            let offset = offset.clamp(0.0, (rows_height(self.items.len()) - height).max(0.0));

            // Lay out one extra card at each edge before it starts entering the viewport.
            let first_visible = ((offset - LIST_PADDING) / ROW_STRIDE).floor().max(0.0) as usize;
            let first = first_visible.saturating_sub(1);
            let end = ((offset + height - LIST_PADDING) / ROW_STRIDE)
                .ceil()
                .max(0.0) as usize;
            let end = (end + 1).min(self.items.len());
            let first_top = LIST_PADDING + first as f32 * ROW_STRIDE - offset;
            let visible = &self.items[first..end];
            self.visible_since
                .retain(|key, _| visible.iter().any(|toast| toast.key == *key));
            let mut rows = Vec::with_capacity(visible.len());
            for (index, toast) in visible.iter().enumerate() {
                let top = first_top + index as f32 * ROW_STRIDE;
                let edge = row_opacity(top, height);
                let opacity = if edge == 0.0 {
                    self.visible_since.remove(&toast.key);
                    0.0
                } else {
                    let entered = *self.visible_since.entry(toast.key).or_insert(now);
                    let fade = fade_in_progress(entered, now);
                    animating |= fade < 1.0;
                    edge * fade
                };
                rows.push(
                    self.row(toast, open_app.clone(), dismiss.clone())
                        .opacity(opacity),
                );
            }
            if animating {
                window.request_animation_frame();
            }
            return panel.on_scroll_wheel(scroll).child(
                div()
                    .absolute()
                    .left(px(12.0))
                    .right(px(12.0))
                    .top(px(first_top))
                    .flex()
                    .flex_col()
                    .gap(px(ROW_GAP))
                    .children(rows),
            );
        }
        self.visible_since.clear();

        let message = match self.access {
            Access::Unpackaged => {
                "Launch the registered development package to read Windows notifications."
            }
            Access::Unavailable => "Windows notification access is unavailable.",
            Access::Denied => {
                "Allow wi-notch in Windows Settings under Privacy & security → Notifications."
            }
            Access::Unspecified if self.requesting => "Waiting for Windows permission…",
            Access::Unspecified => {
                "Allow wi-notch to read the notifications Windows shows from your apps."
            }
            Access::Allowed if !self.initialized => "Checking Windows notifications…",
            Access::Allowed => "No notifications",
        };
        panel.overflow_y_scroll().child(
            div()
                .py(px(8.0))
                .flex()
                .flex_col()
                .items_start()
                .gap(px(6.0))
                .text_size(px(12.0))
                .text_color(rgb(0xb9b9b9))
                .child(message)
                .when(
                    self.access == Access::Unspecified && !self.requesting,
                    |body| {
                        body.child(
                            div()
                                .id("enable-notifications")
                                .px(px(8.0))
                                .py(px(4.0))
                                .rounded_md()
                                .bg(rgb(0x333333))
                                .text_color(rgb(0xf5f5f5))
                                .cursor_pointer()
                                .hover(|style| style.bg(rgb(0x454545)))
                                .child("Enable Windows notifications")
                                .on_click(enable),
                        )
                    },
                ),
        )
    }

    pub fn preview(
        &self,
        toast: &Toast,
        dismiss: impl Fn(ToastKey, &mut App) + 'static,
    ) -> Stateful<Div> {
        let key = toast.key;
        div()
            .id("notification-preview")
            .size_full()
            .px(px(16.0))
            .pr(px(36.0))
            .relative()
            .flex()
            .items_center()
            .gap(px(10.0))
            .cursor_pointer()
            .child(
                div()
                    .relative()
                    .flex_none()
                    .child(self.icon_tile(toast, None))
                    .when(!self.unread.is_empty(), |icon| {
                        icon.child(
                            div()
                                .absolute()
                                .top(px(-2.0))
                                .right(px(-2.0))
                                .size(px(7.0))
                                .rounded_full()
                                .bg(rgb(0xf54b55)),
                        )
                    }),
            )
            .child(message(toast, self.error_message(key), 1))
            .children(self.artwork_thumbnail(key))
            .child(
                div()
                    .flex_none()
                    .text_size(px(10.0))
                    .text_color(rgb(0x9b9b9b))
                    .child("now"),
            )
            .child(dismiss_button(move |cx| dismiss(key, cx)))
    }

    fn row(
        &self,
        toast: &Toast,
        open_app: impl Fn(ToastKey, &mut App) + 'static,
        dismiss: impl Fn(ToastKey, &mut App) + 'static,
    ) -> Stateful<Div> {
        let key = toast.key;
        div()
            .id(SharedString::from(format!(
                "notification-{}-{}",
                key.id, key.created
            )))
            .h(px(ROW_HEIGHT))
            .flex_none()
            .relative()
            .rounded_lg()
            .bg(rgb(0x191919))
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(8.0))
            .pr(px(34.0))
            .child(self.icon_tile(toast, Some(app_accent(toast))))
            .child(message(toast, self.error_message(key), 2))
            .children(self.artwork_thumbnail(key))
            .child(dismiss_button(move |cx| dismiss(key, cx)))
            .when(!toast.app_id.is_empty(), |row| {
                row.cursor_pointer()
                    .hover(|style| style.bg(rgb(0x262626)))
                    .on_click(move |_, _, cx| open_app(key, cx))
            })
    }

    fn error_message(&self, key: ToastKey) -> Option<&'static str> {
        self.action_error
            .filter(|(failed_key, _)| *failed_key == key)
            .map(|(_, message)| message)
    }

    fn artwork_thumbnail(&self, key: ToastKey) -> Option<Img> {
        let image = self.artwork.get(&key)?.as_ref()?;
        Some(
            img(image.clone())
                .w(px(68.0))
                .h(px(48.0))
                .flex_none()
                .rounded_md()
                .object_fit(ObjectFit::Contain),
        )
    }

    fn icon_tile(&self, toast: &Toast, accent: Option<u32>) -> Div {
        let fallback = || {
            toast
                .app_name
                .chars()
                .next()
                .unwrap_or('W')
                .to_uppercase()
                .to_string()
        };
        let tile = div()
            .size(px(24.0))
            .flex_none()
            .rounded_md()
            .text_size(px(12.0))
            .flex()
            .items_center()
            .justify_center();
        match self.icons.get(&toast.app_id).and_then(Option::as_ref) {
            Some(icon) => tile.child(
                img(icon.clone())
                    .size(px(ICON_SIZE as f32))
                    .object_fit(ObjectFit::Contain),
            ),
            None => tile
                .bg(rgb(accent.unwrap_or(0x303030)))
                .when(accent.is_some(), |tile| tile.text_color(rgb(0x151515)))
                .child(fallback()),
        }
    }
}

fn dismiss_button(dismiss: impl Fn(&mut App) + 'static) -> Stateful<Div> {
    div()
        .id("dismiss-notification")
        .absolute()
        .top(px(5.0))
        .right(px(5.0))
        .size(px(25.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .text_size(px(17.0))
        .text_color(rgb(0xcccccc))
        .hover(|style| style.bg(rgb(0x3a3a3a)).text_color(rgb(0xffffff)))
        .child("×")
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            dismiss(cx);
        })
}

fn app_accent(toast: &Toast) -> u32 {
    const COLORS: [u32; 4] = [0x64db88, 0x66acff, 0xbea6f5, 0xf2b974];
    let source = if toast.app_id.is_empty() {
        &toast.app_name
    } else {
        &toast.app_id
    };
    let index = source.bytes().map(usize::from).sum::<usize>() % COLORS.len();
    COLORS[index]
}

pub(super) fn render_icon(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    render_icon_pixels(&decode_image(bytes, 2048)?.into_rgba8())
}

pub(super) fn render_icon_pixels(pixels: &RgbaImage) -> Option<Arc<RenderImage>> {
    let pixels = trim_transparent_padding(pixels)?;
    Some(render_image(resize_icon(pixels)))
}

pub(super) fn render_artwork(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    let pixels = decode_image(bytes, 2048)?.thumbnail(136, 96).into_rgba8();
    Some(render_image(pixels))
}

fn decode_image(bytes: &[u8], max_dimension: u32) -> Option<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(max_dimension);
    limits.max_image_height = Some(max_dimension);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);

    reader.decode().ok()
}

fn render_image(mut pixels: RgbaImage) -> Arc<RenderImage> {
    // GPUI stores render images in BGRA format.
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new([Frame::new(pixels)]))
}

fn resize_icon(mut pixels: RgbaImage) -> RgbaImage {
    // Filter a 2× image before GPUI draws it. Premultiply alpha so transparent
    // pixels don't darken the edges of the logo during downscaling.
    for pixel in pixels.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    let mut resized = DynamicImage::ImageRgba8(pixels)
        .resize(ICON_SIZE * 2, ICON_SIZE * 2, FilterType::Lanczos3)
        .into_rgba8();

    for pixel in resized.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        if alpha > 0 {
            for channel in &mut pixel.0[..3] {
                *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    resized
}

fn trim_transparent_padding(pixels: &RgbaImage) -> Option<RgbaImage> {
    let (mut left, mut top, mut right, mut bottom) = (pixels.width(), pixels.height(), 0, 0);
    for (x, y, pixel) in pixels.enumerate_pixels() {
        if pixel[3] >= 16 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if right <= left || bottom <= top {
        return None;
    }

    Some(crop_imm(pixels, left, top, right - left, bottom - top).to_image())
}

fn message(toast: &Toast, error: Option<&str>, lines: usize) -> Div {
    let subtitle = error.unwrap_or(if toast.body.is_empty() {
        &toast.app_name
    } else {
        &toast.body
    });
    div()
        .flex_1()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .truncate()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(toast.title.clone()),
        )
        .child(
            div()
                .overflow_hidden()
                .line_clamp(lines)
                .text_size(px(11.0))
                .font_weight(FontWeight::NORMAL)
                .text_color(rgb(0xb9b9b9))
                .child(subtitle.to_owned()),
        )
}

#[cfg(test)]
mod tests {
    use super::{
        ICON_SIZE, Notifications, ROW_FADE_DURATION, ROW_HEIGHT, ROW_STRIDE, SCROLL_DURATION,
        ScrollAnimation, fade_in_progress, resize_icon, row_opacity, trim_transparent_padding,
    };
    use crate::windows::notifications::{Access, Toast, ToastKey};
    use ::image::{Rgba, RgbaImage};
    use std::time::{Duration, Instant};

    #[test]
    fn wheel_animation_accumulates_and_reverses_from_the_visible_offset() {
        let start = Instant::now();
        let mut scroll = ScrollAnimation::default();
        assert!(scroll.scroll_by(60.0, 100.0, false, start));
        assert_eq!(scroll.position(start), (0.0, true));

        let next = start + Duration::from_millis(40);
        let visible = scroll.position(next).0;
        assert!((0.0..60.0).contains(&visible));
        assert!(scroll.scroll_by(60.0, 100.0, false, next));
        assert_eq!(scroll.position(next).0, visible);
        assert_eq!(scroll.target, 100.0);

        assert!(scroll.scroll_by(-60.0, 100.0, false, next));
        assert_eq!(scroll.position(next).0, visible);
        assert_eq!(scroll.position(next + SCROLL_DURATION), (0.0, false));
    }

    #[test]
    fn rows_fade_at_both_viewport_edges() {
        let viewport = 300.0;
        assert_eq!(row_opacity(-ROW_HEIGHT, viewport), 0.0);
        assert_eq!(row_opacity(-ROW_HEIGHT / 2.0, viewport), 0.5);
        assert_eq!(row_opacity(0.0, viewport), 1.0);
        assert_eq!(row_opacity(viewport - ROW_HEIGHT, viewport), 1.0);
        assert_eq!(row_opacity(viewport - ROW_HEIGHT / 2.0, viewport), 0.5);
        assert_eq!(row_opacity(viewport, viewport), 0.0);
    }

    #[test]
    fn entering_rows_keep_fading_after_the_wheel_stops() {
        let start = Instant::now();
        assert_eq!(fade_in_progress(start, start), 0.0);
        assert!(fade_in_progress(start, start + SCROLL_DURATION) < 1.0);
        assert_eq!(fade_in_progress(start, start + ROW_FADE_DURATION), 1.0);
    }

    #[test]
    fn a_new_notification_keeps_the_scrolled_rows_in_place() {
        let mut notifications = Notifications::new(Access::Allowed);
        let mut items: Vec<_> = (1..=12).rev().map(|id| toast(id, id.into())).collect();
        notifications.sync(items.clone(), true);
        let start = Instant::now();
        let offset = 2.5 * ROW_STRIDE;
        notifications.scroll.scroll_by(offset, 300.0, true, start);

        items.insert(0, toast(13, 13));
        notifications.sync(items, true);
        assert_eq!(
            notifications.scroll.position(start),
            (offset + ROW_STRIDE, false)
        );
    }

    fn toast(id: u32, created: i64) -> Toast {
        Toast {
            key: ToastKey { id, created },
            app_name: "Test".into(),
            app_id: String::new(),
            title: "Message".into(),
            body: String::new(),
        }
    }

    #[test]
    fn app_logo_uses_its_visible_pixels_instead_of_the_padded_canvas() {
        let mut logo = RgbaImage::new(150, 150);
        assert!(trim_transparent_padding(&logo).is_none());
        logo.put_pixel(0, 0, Rgba([255, 255, 255, 1]));
        let green = Rgba([0, 200, 100, 255]);
        for x in 50..105 {
            for y in 45..100 {
                logo.put_pixel(x, y, green);
            }
        }

        let cropped = trim_transparent_padding(&logo).unwrap();
        assert_eq!(cropped.dimensions(), (55, 55));
        assert!(cropped.pixels().all(|pixel| *pixel == green));
    }

    #[test]
    fn icon_downscaling_preserves_aspect_ratio_and_edge_color() {
        let mut logo = RgbaImage::new(72, 36);
        for x in 0..36 {
            for y in 0..36 {
                logo.put_pixel(x, y, Rgba([0, 255, 0, 255]));
            }
        }
        let resized = resize_icon(logo);
        assert_eq!(resized.dimensions(), (ICON_SIZE * 2, ICON_SIZE));
        assert!(resized.pixels().any(|pixel| pixel[3] > 0 && pixel[3] < 255));
        assert!(
            resized
                .pixels()
                .filter(|pixel| pixel[3] > 0)
                .all(|pixel| pixel[1] == 255)
        );
    }

    #[test]
    fn arrivals_are_reported_once_and_visible_items_stay_read() {
        let mut list = Notifications::new(Access::Allowed);
        assert_eq!(list.sync(vec![toast(1, 10)], false), (true, None));
        assert_eq!(list.sync(vec![toast(1, 10)], false), (false, None));
        assert_eq!(
            list.sync(vec![toast(2, 20), toast(1, 10)], true),
            (true, Some(ToastKey { id: 2, created: 20 }))
        );
        assert!(list.unread.is_empty());
        assert_eq!(
            list.sync(vec![toast(2, 20), toast(1, 10)], false),
            (false, None)
        );
    }

    #[test]
    fn stale_poll_does_not_restore_a_dismissed_card() {
        let mut list = Notifications::new(Access::Allowed);
        let selected = toast(1, 10);
        let other = toast(2, 20);
        list.sync(vec![other.clone(), selected.clone()], true);

        list.dismiss(selected.key);
        list.sync(vec![other.clone(), selected.clone()], true);
        assert_eq!(list.items, [other.clone()]);

        let replacement = toast(selected.key.id, 30);
        list.sync(vec![replacement.clone(), other.clone()], true);
        assert_eq!(list.items, [replacement, other]);
        assert!(list.dismissed.is_empty());
    }

    #[test]
    fn bursts_keep_all_rows_and_reused_ids_count_as_new() {
        let mut list = Notifications::new(Access::Allowed);
        list.sync(vec![toast(1, 10)], false);
        let items = (1..=8)
            .rev()
            .map(|id| toast(id, 30 + i64::from(id)))
            .collect();
        assert_eq!(
            list.sync(items, false),
            (true, Some(ToastKey { id: 8, created: 38 }))
        );
        assert_eq!(list.unread.len(), 8);
        assert_eq!(list.items.len(), 8);
        list.open();
        assert!(list.unread.is_empty());
    }

    #[test]
    fn revoked_access_clears_messages_and_starts_a_new_baseline() {
        let mut list = Notifications::new(Access::Allowed);
        list.sync(vec![toast(1, 10)], false);
        list.sync(vec![toast(2, 20), toast(1, 10)], false);
        assert!(!list.items.is_empty());
        assert!(!list.unread.is_empty());
        assert!(list.set_access(Access::Denied, drop));
        assert!(list.items.is_empty());
        assert!(list.unread.is_empty());
        assert!(list.set_access(Access::Allowed, drop));
        assert_eq!(list.sync(vec![toast(2, 20)], false), (true, None));
    }
}
