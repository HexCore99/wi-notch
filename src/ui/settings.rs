mod appearance;
mod display;

use appearance::AppearancePage;
use display::DisplayPage;
use gpui::{prelude::*, *};

use crate::appearance::AppearanceSettings;
use crate::windows::window::remove_window_border;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Appearance,
    Display,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Display => "Display",
        }
    }
}

pub struct SettingsView {
    page: Page,
    appearance_page: Entity<AppearancePage>,
    display_page: Entity<DisplayPage>,
}

impl SettingsView {
    pub fn open(
        appearance: Entity<AppearanceSettings>,
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
                    page: Page::Appearance,
                    appearance_page: cx.new(|_| AppearancePage::new(appearance.clone())),
                    display_page: cx.new(|_| DisplayPage::new(appearance)),
                })
            },
        )
    }

    fn sidebar_item(&self, page: Page, icon: &'static str, cx: &Context<Self>) -> Stateful<Div> {
        let selected = self.page == page;
        div()
            .id(page.title())
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(8.0))
            .rounded_md()
            .cursor_pointer()
            .when(selected, |item| {
                item.bg(rgb(0x29364e)).text_color(rgb(0xcbdcff))
            })
            .when(!selected, |item| {
                item.hover(|style| style.bg(rgb(0x212a39)))
            })
            .child(
                div()
                    .size(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .bg(rgb(0x344766))
                    .font_family("Segoe UI Symbol")
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
            Page::Appearance => self.appearance_page.clone().into_any_element(),
            Page::Display => self.display_page.clone().into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .rounded_xl()
            .overflow_hidden()
            .bg(rgb(0x1b2230))
            .text_color(rgb(0xe9eef6))
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
                    .bg(rgb(0x121824))
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
                    .child(self.sidebar_item(Page::Appearance, "◐", cx))
                    .child(self.sidebar_item(Page::Display, "▣", cx)),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
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
                                    .text_color(rgb(0xb9c5d7))
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

fn update_settings(
    appearance: &Entity<AppearanceSettings>,
    change: impl FnOnce(&mut AppearanceSettings),
    cx: &mut App,
) -> std::io::Result<()> {
    let settings = appearance.update(cx, |settings, cx| {
        let previous = *settings;
        change(settings);
        if *settings != previous {
            cx.notify();
        }
        *settings
    });
    settings.save()
}

fn setting_row(label: &'static str, control: impl IntoElement) -> Div {
    div()
        .h(px(44.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .justify_between()
        .rounded_md()
        .bg(rgb(0x232c3b))
        .child(label)
        .child(control)
}

fn save_error() -> Div {
    div()
        .text_size(px(12.0))
        .text_color(rgb(0xffa59c))
        .child("Couldn’t save settings. Changes apply for this session.")
}
