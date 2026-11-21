use gpui::{prelude::*, *};

use super::{save_error, setting_row, setting_switch, update_settings};
use crate::appearance::{
    AppearanceSettings, MAX_IDLE_COLLAPSE_DELAY_SECS, MIN_IDLE_COLLAPSE_DELAY_SECS,
};

pub(super) struct GeneralPage {
    settings: Entity<AppearanceSettings>,
    save_failed: bool,
}

impl GeneralPage {
    pub(super) fn new(settings: Entity<AppearanceSettings>) -> Self {
        Self {
            settings,
            save_failed: false,
        }
    }

    fn update_settings(
        &mut self,
        change: impl FnOnce(&mut AppearanceSettings),
        cx: &mut Context<Self>,
    ) {
        self.save_failed = update_settings(&self.settings, change, cx).is_err();
        cx.notify();
    }

    fn delay_button(increase: bool, enabled: bool, cx: &Context<Self>) -> Stateful<Div> {
        let label = if increase { "+" } else { "−" };
        div()
            .id(label)
            .size(px(26.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .bg(rgb(0x34445d))
            .text_size(px(17.0))
            .child(label)
            .when(!enabled, |button| button.opacity(0.4))
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0x405675)))
                    .on_click(cx.listener(move |page, _, _, cx| {
                        page.update_settings(
                            |settings| {
                                let seconds = settings.idle_collapse_seconds();
                                let next = if increase { seconds + 5 } else { seconds - 5 };
                                settings.idle_collapse_delay_secs = next.clamp(
                                    MIN_IDLE_COLLAPSE_DELAY_SECS,
                                    MAX_IDLE_COLLAPSE_DELAY_SECS,
                                );
                            },
                            cx,
                        );
                    }))
            })
    }
}

impl Render for GeneralPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = self.settings.read(cx);
        let auto = settings.auto_minimal;
        let always = settings.always_minimal;
        let seconds = settings.idle_collapse_seconds();
        let auto_switch =
            setting_switch("auto-minimal", auto).on_click(cx.listener(|page, _, _, cx| {
                page.update_settings(
                    |settings| settings.auto_minimal = !settings.auto_minimal,
                    cx,
                );
            }));
        let always_switch =
            setting_switch("always-minimal", always).on_click(cx.listener(|page, _, _, cx| {
                page.update_settings(
                    |settings| settings.always_minimal = !settings.always_minimal,
                    cx,
                );
            }));
        let delay = div()
            .flex()
            .items_center()
            .child(Self::delay_button(
                false,
                auto && seconds > MIN_IDLE_COLLAPSE_DELAY_SECS,
                cx,
            ))
            .child(
                div()
                    .w(px(70.0))
                    .text_center()
                    .child(format!("{seconds} s")),
            )
            .child(Self::delay_button(
                true,
                auto && seconds < MAX_IDLE_COLLAPSE_DELAY_SECS,
                cx,
            ));

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(setting_row("Auto minimal notch", auto_switch))
            .child(setting_row("Idle collapse delay", delay))
            .child(setting_row("Always minimal", always_switch))
            .when(always, |page| {
                page.child(
                    div()
                        .px(px(12.0))
                        .text_size(px(12.0))
                        .text_color(rgb(0xb9c5d7))
                        .child("Hover over the strip to open the player."),
                )
            })
            .when(self.save_failed, |page| page.child(save_error()))
    }
}
