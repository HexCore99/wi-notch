mod appearance;
mod display;
mod general;
mod pomodoro;
mod prayer;
mod text_input;

use appearance::AppearancePage;
use display::DisplayPage;
use general::GeneralPage;
use gpui::{prelude::*, *};
use pomodoro::PomodoroPage;
use prayer::PrayerPage;

use crate::appearance::AppearanceSettings;
use crate::icons::tomato;
use crate::windows::window::remove_window_border;

const CONTROL_BACKGROUND: u32 = 0x3a3a3c;
const CONTROL_HOVER: u32 = 0x4a4a4d;
const MUTED_TEXT: u32 = 0xa8a8aa;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    General,
    Appearance,
    Display,
    PrayerTimes,
    Pomodoro,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Display => "Display",
            Self::PrayerTimes => "Prayer Times",
            Self::Pomodoro => "Pomodoro",
        }
    }
}

pub struct SettingsView {
    page: Page,
    general_page: Entity<GeneralPage>,
    appearance_page: Entity<AppearancePage>,
    display_page: Entity<DisplayPage>,
    prayer_page: Entity<PrayerPage>,
    pomodoro_page: Entity<PomodoroPage>,
}

impl SettingsView {
    pub fn open(
        appearance: Entity<AppearanceSettings>,
        page: Page,
        cx: &mut App,
    ) -> Result<WindowHandle<Self>> {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(652.0), px(556.0)), cx)),
                titlebar: Some(TitlebarOptions {
                    title: Some("wi-notch Settings".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                is_resizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            |window, cx| {
                if let Err(error) = remove_window_border(window) {
                    eprintln!("Failed to remove the Settings window border: {error}");
                }
                cx.new(|cx| Self {
                    page,
                    general_page: cx.new(|_| GeneralPage::new(appearance.clone())),
                    appearance_page: cx.new(|_| AppearancePage::new(appearance.clone())),
                    display_page: cx.new(|_| DisplayPage::new(appearance.clone())),
                    prayer_page: cx.new(|cx| PrayerPage::new(appearance.clone(), cx)),
                    pomodoro_page: cx.new(|cx| PomodoroPage::new(appearance, cx)),
                })
            },
        )
    }

    pub(super) fn show_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    fn sidebar_item(&self, page: Page, cx: &Context<Self>) -> Stateful<Div> {
        let selected = self.page == page;
        let (icon, badge_color) = match page {
            Page::General => (
                div()
                    .font_family("Segoe UI Symbol")
                    .text_size(px(14.0))
                    .text_color(rgb(0xffffff))
                    .child("⚙")
                    .into_any_element(),
                rgb(0x4f7eca),
            ),
            Page::Appearance => (sidebar_icon("circle-half.svg"), rgb(0x56627a)),
            Page::Display => (sidebar_icon("display-fill.svg"), rgb(0x5454d4)),
            Page::PrayerTimes => (
                div()
                    .font_family("Segoe UI Symbol")
                    .text_size(px(14.0))
                    .child("☾")
                    .into_any_element(),
                rgb(0x6b49a6),
            ),
            Page::Pomodoro => (tomato(14.0).into_any_element(), rgb(0x4e2d4d)),
        };
        div()
            .id(page.title())
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(8.0))
            .rounded_md()
            .cursor_pointer()
            .when(selected, |item| item.bg(rgb(0x2a2a2c)))
            .when(!selected, |item| {
                item.hover(|style| style.bg(rgb(0x1e1e20)))
            })
            .child(
                div()
                    .size(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .bg(badge_color)
                    .child(icon),
            )
            .child(page.title())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.page = page;
                cx.notify();
            }))
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page {
            Page::General => self.general_page.clone().into_any_element(),
            Page::Appearance => self.appearance_page.clone().into_any_element(),
            Page::Display => self.display_page.clone().into_any_element(),
            Page::PrayerTimes => self.prayer_page.clone().into_any_element(),
            Page::Pomodoro => self.pomodoro_page.clone().into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .rounded_xl()
            .overflow_hidden()
            .bg(rgb(0x1c1c1e))
            .text_color(rgb(0xf4f4f5))
            .text_size(px(13.0))
            .child(
                div()
                    .w(px(192.0))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .px(px(10.0))
                    .rounded_l_xl()
                    .bg(rgb(0x151517))
                    .child(
                        div()
                            .h(px(61.0))
                            .flex()
                            .items_center()
                            .px(px(12.0))
                            .window_control_area(WindowControlArea::Drag)
                            .text_size(px(16.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Settings"),
                    )
                    .child(self.sidebar_item(Page::General, cx))
                    .child(self.sidebar_item(Page::Appearance, cx))
                    .child(self.sidebar_item(Page::Display, cx))
                    .child(self.sidebar_item(Page::PrayerTimes, cx))
                    .child(self.sidebar_item(Page::Pomodoro, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .px(px(22.0))
                    .child(
                        div()
                            .h(px(60.0))
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .window_control_area(WindowControlArea::Drag)
                                    .text_size(px(22.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.page.title()),
                            )
                            .child(
                                div()
                                    .id("close-settings")
                                    .size(px(32.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .text_color(rgb(MUTED_TEXT))
                                    .text_size(px(22.0))
                                    .cursor_pointer()
                                    .hover(|style| {
                                        style.bg(rgb(0x70343d)).text_color(rgb(0xffffff))
                                    })
                                    .child("×")
                                    .on_click(|_, window, _| window.remove_window()),
                            ),
                    )
                    .child(content),
            )
    }
}

fn sidebar_icon(path: &'static str) -> AnyElement {
    svg()
        .path(path)
        .size(px(12.0))
        .text_color(rgb(0xffffff))
        .into_any_element()
}

fn update_settings(
    appearance: &Entity<AppearanceSettings>,
    change: impl FnOnce(&mut AppearanceSettings),
    cx: &mut App,
) -> std::io::Result<()> {
    appearance.update(cx, |settings, cx| {
        let previous = settings.clone();
        change(settings);
        if *settings != previous {
            cx.notify();
        }
        settings.save()
    })
}

fn setting_group(title: &'static str, rows: impl IntoIterator<Item = Div>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .px(px(6.0))
                .text_size(px(11.0))
                .text_color(rgb(MUTED_TEXT))
                .child(title),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .rounded_xl()
                .overflow_hidden()
                .bg(rgb(0x28282a))
                .children(rows.into_iter().enumerate().map(|(index, row)| {
                    row.when(index > 0, |row| {
                        row.border_t_1().border_color(rgb(0x303032))
                    })
                })),
        )
}

fn setting_row(label: impl IntoElement, control: impl IntoElement) -> Div {
    div()
        .h(px(44.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .justify_between()
        .child(label)
        .child(control)
}

fn setting_switch(id: &'static str, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(34.0))
        .h(px(20.0))
        .px(px(3.0))
        .flex()
        .items_center()
        .when(on, |toggle| toggle.justify_end())
        .rounded_full()
        .bg(rgb(if on { 0x30c759 } else { 0x47474a }))
        .cursor_pointer()
        .child(div().size(px(14.0)).rounded_full().bg(rgb(0xffffff)))
}

fn save_error() -> Div {
    div()
        .text_size(px(12.0))
        .text_color(rgb(0xffa59c))
        .child("Couldn’t save settings. Changes apply for this session.")
}
