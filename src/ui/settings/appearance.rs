use gpui::{prelude::*, *};

use super::{save_error, setting_row, update_settings};
use crate::appearance::{AppearanceSettings, Background, Corners};

pub(super) struct AppearancePage {
    appearance: Entity<AppearanceSettings>,
    save_failed: bool,
}

impl AppearancePage {
    pub(super) fn new(appearance: Entity<AppearanceSettings>) -> Self {
        Self {
            appearance,
            save_failed: false,
        }
    }

    fn swatch(
        label: &'static str,
        color: Background,
        current: Background,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(label)
            .size(px(26.0))
            .bg(rgb(color.color()))
            .border_2()
            .border_color(rgb(if color == current { 0x9bbcff } else { 0x515e73 }))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.update_settings(|settings| settings.background = color, cx);
            }))
    }

    fn update_settings(
        &mut self,
        change: impl FnOnce(&mut AppearanceSettings),
        cx: &mut Context<Self>,
    ) {
        self.save_failed = update_settings(&self.appearance, change, cx).is_err();
        cx.notify();
    }
}

impl Render for AppearancePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = *self.appearance.read(cx);
        let rounded = appearance.corners == Corners::Rounded;
        let background = div()
            .flex()
            .gap(px(8.0))
            .child(Self::swatch(
                "black",
                Background::Black,
                appearance.background,
                cx,
            ))
            .child(Self::swatch(
                "charcoal",
                Background::Charcoal,
                appearance.background,
                cx,
            ));
        let corners = div()
            .id("corner-rounding")
            .w(px(34.0))
            .h(px(20.0))
            .px(px(3.0))
            .flex()
            .items_center()
            .when(rounded, |toggle| toggle.justify_end())
            .rounded_full()
            .bg(rgb(if rounded { 0x4f7eca } else { 0x465268 }))
            .cursor_pointer()
            .child(div().size(px(14.0)).rounded_full().bg(rgb(0xecf3ff)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.update_settings(
                    |settings| {
                        settings.corners = match settings.corners {
                            Corners::Rounded => Corners::Square,
                            Corners::Square => Corners::Rounded,
                        };
                    },
                    cx,
                );
            }));

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(setting_row("Background", background))
            .child(setting_row("Corner rounding", corners))
            .when(self.save_failed, |page| page.child(save_error()))
    }
}
