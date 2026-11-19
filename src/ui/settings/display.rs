use gpui::{prelude::*, *};

use super::{save_error, setting_row, update_settings};
use crate::appearance::AppearanceSettings;
use crate::notch_geometry::{TOP_GAP_STEP, max_top_gap};

pub(super) struct DisplayPage {
    appearance: Entity<AppearanceSettings>,
    save_failed: bool,
}

impl DisplayPage {
    pub(super) fn new(appearance: Entity<AppearanceSettings>) -> Self {
        Self {
            appearance,
            save_failed: false,
        }
    }

    fn step_button(
        label: &'static str,
        amount: i32,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
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
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let limit = gap_limit(cx);
                        this.save_failed = update_settings(
                            &this.appearance,
                            |settings| {
                                let gap = settings.top_gap.min(limit);
                                settings.top_gap = gap.saturating_add_signed(amount).min(limit);
                            },
                            cx,
                        )
                        .is_err();
                        cx.notify();
                    }))
            })
    }
}

impl Render for DisplayPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let limit = gap_limit(cx);
        let gap = self.appearance.read(cx).top_gap.min(limit);
        let step = TOP_GAP_STEP as i32;
        let controls = div()
            .flex()
            .items_center()
            .child(Self::step_button("−", -step, gap > 0, cx))
            .child(div().w(px(70.0)).text_center().child(format!("{gap} px")))
            .child(Self::step_button("+", step, gap < limit, cx));

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(setting_row("Top gap", controls))
            .when(self.save_failed, |page| page.child(save_error()))
    }
}

fn gap_limit(cx: &App) -> u32 {
    cx.primary_display()
        .map_or(0, |display| max_top_gap(display.bounds().size.height))
}
