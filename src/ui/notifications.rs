use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    sync::Arc,
};

use ::image::{
    DynamicImage, Frame, ImageReader, Limits, RgbaImage,
    imageops::{FilterType, crop_imm},
};
use gpui::{prelude::*, *};

use crate::windows::notifications::{Access, Toast, ToastKey};

const ROW_HEIGHT: f32 = 44.0;
const ROW_GAP: f32 = 6.0;
const LIST_PADDING: f32 = 4.0;
const MAX_ROWS: usize = 6;
const ICON_SIZE: u32 = 18;
pub const PREVIEW_HEIGHT: f32 = 64.0;

pub struct Notifications {
    pub access: Access,
    pub requesting: bool,
    pub icons: HashMap<String, Option<Arc<RenderImage>>>,
    pub launch_error: Option<ToastKey>,
    scroll: ScrollHandle,
    items: Vec<Toast>,
    unread: HashSet<ToastKey>,
    initialized: bool,
}

impl Notifications {
    pub fn new(access: Access) -> Self {
        Self {
            access,
            requesting: false,
            icons: HashMap::new(),
            launch_error: None,
            scroll: ScrollHandle::new(),
            items: Vec::new(),
            unread: HashSet::new(),
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
            self.launch_error = None;
            self.initialized = false;
            for image in self.icons.drain().filter_map(|(_, image)| image) {
                drop_image(image);
            }
        }
        true
    }

    pub fn sync(&mut self, items: Vec<Toast>, open: bool) -> (bool, Option<ToastKey>) {
        let changed = !self.initialized || self.items != items;
        if !changed {
            return (false, None);
        }

        let current: HashSet<_> = items.iter().map(|toast| toast.key).collect();
        self.unread.retain(|key| current.contains(key));
        let mut newest = None;
        if self.initialized && !open {
            let previous: HashSet<_> = self.items.iter().map(|toast| toast.key).collect();
            for toast in items.iter().filter(|toast| !previous.contains(&toast.key)) {
                newest.get_or_insert(toast.key);
                self.unread.insert(toast.key);
            }
        }
        self.initialized = true;
        self.items = items;
        (true, newest)
    }

    pub fn open(&mut self) {
        self.unread.clear();
        self.launch_error = None;
        self.scroll.set_offset(point(px(0.0), px(0.0)));
    }

    pub fn find(&self, key: ToastKey) -> Option<&Toast> {
        self.items.iter().find(|toast| toast.key == key)
    }

    pub fn list_height(&self) -> f32 {
        if self.items.is_empty() {
            84.0
        } else {
            let rows = self.items.len().min(MAX_ROWS) as f32;
            rows * ROW_HEIGHT + (rows - 1.0) * ROW_GAP + 2.0 * LIST_PADDING
        }
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
        &self,
        height: f32,
        enable: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        open_app: impl Fn(ToastKey, &mut App) + Clone + 'static,
    ) -> Stateful<Div> {
        let panel = div()
            .id("notifications-scroll")
            .w_full()
            .h(px(height.max(0.0)))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .px(px(12.0));
        if self.access == Access::Allowed && !self.items.is_empty() {
            return panel
                .flex()
                .flex_col()
                .gap(px(ROW_GAP))
                .py(px(LIST_PADDING))
                .children(
                    self.items
                        .iter()
                        .map(|toast| self.row(toast, open_app.clone())),
                );
        }

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
        panel.child(
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

    pub fn preview(&self, toast: &Toast) -> Stateful<Div> {
        div()
            .id("notification-preview")
            .size_full()
            .px(px(16.0))
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
            .child(message(toast, false))
            .child(
                div()
                    .flex_none()
                    .text_size(px(10.0))
                    .text_color(rgb(0x9b9b9b))
                    .child("now"),
            )
    }

    fn row(&self, toast: &Toast, open_app: impl Fn(ToastKey, &mut App) + 'static) -> Stateful<Div> {
        let key = toast.key;
        div()
            .id(SharedString::from(format!(
                "notification-{}-{}",
                key.id, key.created
            )))
            .h(px(ROW_HEIGHT))
            .flex_none()
            .rounded_lg()
            .bg(rgb(0x191919))
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(8.0))
            .child(self.icon_tile(toast, Some(app_accent(toast))))
            .child(message(toast, self.launch_error == Some(key)))
            .when(!toast.app_id.is_empty(), |row| {
                row.cursor_pointer()
                    .hover(|style| style.bg(rgb(0x262626)))
                    .on_click(move |_, _, cx| open_app(key, cx))
            })
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
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(1024);
    limits.max_image_height = Some(1024);
    limits.max_alloc = Some(16 * 1024 * 1024);
    reader.limits(limits);

    let pixels = reader.decode().ok()?.into_rgba8();
    let pixels = trim_transparent_padding(&pixels)?;
    let mut pixels = resize_icon(pixels);
    // GPUI stores render images in BGRA format.
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new([Frame::new(pixels)])))
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

fn message(toast: &Toast, launch_failed: bool) -> Div {
    let subtitle = if launch_failed {
        "Windows couldn’t open this app."
    } else if toast.body.is_empty() {
        &toast.app_name
    } else {
        &toast.body
    };
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
                .truncate()
                .text_size(px(11.0))
                .font_weight(FontWeight::NORMAL)
                .text_color(rgb(0xb9b9b9))
                .child(subtitle.to_owned()),
        )
}

#[cfg(test)]
mod tests {
    use super::{ICON_SIZE, Notifications, resize_icon, trim_transparent_padding};
    use crate::windows::notifications::{Access, Toast, ToastKey};
    use ::image::{Rgba, RgbaImage};

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
    fn baseline_repeated_reads_and_open_list_do_not_produce_arrivals() {
        let mut list = Notifications::new(Access::Allowed);
        assert_eq!(list.sync(vec![toast(1, 10)], false), (true, None));
        assert_eq!(list.sync(vec![toast(1, 10)], false), (false, None));
        assert_eq!(
            list.sync(vec![toast(2, 20), toast(1, 10)], true),
            (true, None)
        );
        assert!(list.unread.is_empty());
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
