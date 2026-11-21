use gpui::{prelude::*, *};

use super::{
    CONTROL_BACKGROUND, CONTROL_HOVER, save_error, setting_group, setting_row, update_settings,
};
use crate::appearance::AppearanceSettings;
use crate::notch_geometry::max_top_gap;
use crate::windows::display::{Monitor, monitors, selected_monitor};

pub(super) struct DisplayPage {
    appearance: Entity<AppearanceSettings>,
    save_failed: bool,
    monitor_list_open: bool,
}

impl DisplayPage {
    pub(super) fn new(appearance: Entity<AppearanceSettings>) -> Self {
        Self {
            appearance,
            save_failed: false,
            monitor_list_open: false,
        }
    }

    fn step_button(
        label: &'static str,
        enabled: bool,
        change: impl Fn(&mut AppearanceSettings) + 'static,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(label)
            .size(px(26.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .bg(rgb(CONTROL_BACKGROUND))
            .text_size(px(17.0))
            .child(label)
            .when(!enabled, |button| button.opacity(0.4))
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(CONTROL_HOVER)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.update_settings(|settings| change(settings), cx);
                    }))
            })
    }

    fn stepper(
        id: &'static str,
        value: u32,
        min: u32,
        max: u32,
        change: impl Fn(&mut AppearanceSettings, bool) + Copy + 'static,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .flex()
            .items_center()
            .child(Self::step_button(
                "−",
                value > min,
                move |settings| change(settings, false),
                cx,
            ))
            .child(div().w(px(70.0)).text_center().child(format!("{value} px")))
            .child(Self::step_button(
                "+",
                value < max,
                move |settings| change(settings, true),
                cx,
            ))
    }

    fn monitor_picker(
        &self,
        monitor: Option<&Monitor>,
        follow_primary: bool,
        cx: &Context<Self>,
    ) -> Div {
        let label = match monitor {
            Some(_) if follow_primary => "Follow Windows primary".into(),
            Some(monitor) => monitor.label(),
            None => "No display available".into(),
        };
        setting_row(
            "Monitor",
            div()
                .id("monitor-picker")
                .flex()
                .items_center()
                .gap(px(10.0))
                .px(px(9.0))
                .py(px(5.0))
                .rounded_md()
                .bg(rgb(CONTROL_BACKGROUND))
                .child(label)
                .when(monitor.is_some(), |button| {
                    button
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(CONTROL_HOVER)))
                        .child(if self.monitor_list_open { "▴" } else { "▾" })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.monitor_list_open = !this.monitor_list_open;
                            cx.notify();
                        }))
                }),
        )
    }

    fn monitor_option(
        label: impl Into<SharedString>,
        device: Option<&str>,
        selected: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let id = SharedString::from(device.unwrap_or("primary").to_owned());
        let device = device.map(str::to_owned);
        div()
            .id(id)
            .px(px(10.0))
            .py(px(6.0))
            .rounded_md()
            .cursor_pointer()
            .when(selected, |item| item.bg(rgb(CONTROL_BACKGROUND)))
            .when(!selected, |item| {
                item.hover(|style| style.bg(rgb(CONTROL_HOVER)))
            })
            .child(label.into())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.monitor_list_open = false;
                this.update_settings(|settings| settings.monitor = device.clone(), cx);
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

impl Render for DisplayPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = self.appearance.read(cx);
        let follow_primary = settings.monitor.is_none();
        let monitors = monitors();
        let monitor = selected_monitor(&monitors, settings.monitor.as_deref());
        let limit = monitor.map_or(0, |monitor| max_top_gap(monitor.bounds.size.height));
        let offset_limit = limit.max(1);
        let gap = Self::stepper(
            "top-gap",
            settings.top_gap.min(limit),
            0,
            limit,
            move |settings, increase| {
                settings.top_gap = stepped(
                    settings.top_gap,
                    settings.offset.clamp(1, offset_limit),
                    increase,
                    0,
                    limit,
                );
            },
            cx,
        );
        let offset = Self::stepper(
            "offset",
            settings.offset.clamp(1, offset_limit),
            1,
            offset_limit,
            move |settings, increase| {
                settings.offset = stepped(settings.offset, 1, increase, 1, offset_limit);
            },
            cx,
        );

        let monitor_picker = self.monitor_picker(monitor, follow_primary, cx);
        let monitor_options = (self.monitor_list_open && !monitors.is_empty()).then(|| {
            div().p(px(5.0)).child(
                div()
                    .id("monitor-options")
                    .max_h(px(165.0))
                    .overflow_y_scroll()
                    .child(Self::monitor_option(
                        "Follow Windows primary",
                        None,
                        follow_primary,
                        cx,
                    ))
                    .children(monitors.iter().map(|item| {
                        Self::monitor_option(
                            item.label(),
                            Some(item.identity()),
                            !follow_primary && monitor == Some(item),
                            cx,
                        )
                    })),
            )
        });

        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(setting_group(
                "Position",
                [setting_row("Top gap", gap), setting_row("Offset", offset)],
            ))
            .child(setting_group(
                "Display selection",
                std::iter::once(monitor_picker).chain(monitor_options),
            ))
            .when(self.save_failed, |page| page.child(save_error()))
    }
}

fn stepped(value: u32, amount: u32, increase: bool, min: u32, max: u32) -> u32 {
    let value = value.clamp(min, max);
    let next = if increase {
        value.saturating_add(amount)
    } else {
        value.saturating_sub(amount)
    };
    next.clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::stepped;

    #[test]
    fn configurable_offset_steps_and_clamps_the_gap() {
        assert_eq!(stepped(20, 7, true, 0, 100), 27);
        assert_eq!(stepped(20, 7, false, 0, 100), 13);
        assert_eq!(stepped(4, 7, false, 0, 100), 0);
        assert_eq!(stepped(97, 7, true, 0, 100), 100);
        assert_eq!(stepped(0, 1, false, 1, 100), 1);
    }
}
