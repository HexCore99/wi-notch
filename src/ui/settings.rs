use gpui::{prelude::*, *};

use crate::appearance::{AppearanceSettings, Background, Corners};

pub struct SettingsView {
    appearance: Entity<AppearanceSettings>,
    save_failed: bool,
}

impl SettingsView {
    pub fn open(
        appearance: Entity<AppearanceSettings>,
        cx: &mut App,
    ) -> Result<WindowHandle<Self>> {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(360.0), px(220.0)), cx)),
                titlebar: Some(TitlebarOptions {
                    title: Some("wi-notch Settings".into()),
                    ..Default::default()
                }),
                is_resizable: false,
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| Self {
                    appearance,
                    save_failed: false,
                })
            },
        )
    }

    fn choice(
        label: &'static str,
        selected: bool,
        change: impl Fn(&mut AppearanceSettings) + 'static,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(label)
            .px(px(10.0))
            .py(px(6.0))
            .rounded_md()
            .text_size(px(13.0))
            .bg(rgb(if selected { 0xf0f0f0 } else { 0x303030 }))
            .text_color(rgb(if selected { 0x171717 } else { 0xf0f0f0 }))
            .cursor_pointer()
            .when(!selected, |button| {
                button.hover(|style| style.bg(rgb(0x3d3d3d)))
            })
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                let settings = this.appearance.update(cx, |settings, cx| {
                    let previous = *settings;
                    change(settings);
                    if *settings != previous {
                        cx.notify();
                    }
                    *settings
                });
                this.save_failed = settings.save().is_err();
                cx.notify();
            }))
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = *self.appearance.read(cx);
        let background_choices = div()
            .flex()
            .gap(px(6.0))
            .child(Self::choice(
                "Black",
                appearance.background == Background::Black,
                |settings| {
                    settings.background = Background::Black;
                },
                cx,
            ))
            .child(Self::choice(
                "Charcoal",
                appearance.background == Background::Charcoal,
                |settings| {
                    settings.background = Background::Charcoal;
                },
                cx,
            ));
        let corner_choices = div()
            .flex()
            .gap(px(6.0))
            .child(Self::choice(
                "Rounded",
                appearance.corners == Corners::Rounded,
                |settings| {
                    settings.corners = Corners::Rounded;
                },
                cx,
            ))
            .child(Self::choice(
                "Square",
                appearance.corners == Corners::Square,
                |settings| {
                    settings.corners = Corners::Square;
                },
                cx,
            ));

        div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(18.0))
            .gap(px(15.0))
            .bg(rgb(0x171717))
            .text_color(rgb(0xf0f0f0))
            .text_size(px(14.0))
            .child(
                div()
                    .text_size(px(19.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Appearance"),
            )
            .child(setting_row("Background", background_choices))
            .child(setting_row("Corners", corner_choices))
            .when(self.save_failed, |view| {
                view.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(0xffa59c))
                        .child("Couldn’t save settings. Changes apply for this session."),
                )
            })
    }
}

fn setting_row(label: &'static str, choices: Div) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(label)
        .child(choices)
}
