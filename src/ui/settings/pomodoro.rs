use gpui::{prelude::*, *};

use super::{
    MUTED_TEXT,
    text_input::{TextChanged, TextInput},
};
use crate::appearance::{AppearanceSettings, DEFAULT_POMODORO_LABEL};

pub(super) struct PomodoroPage {
    settings: Entity<AppearanceSettings>,
    input: Entity<TextInput>,
    error: Option<&'static str>,
}

impl PomodoroPage {
    pub(super) fn new(settings: Entity<AppearanceSettings>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new("pomodoro-session-name", "Session name", cx));
        cx.subscribe(&input, |page, _, _: &TextChanged, cx| {
            page.error = None;
            cx.notify();
        })
        .detach();
        Self {
            settings,
            input,
            error: None,
        }
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        let name = self.input.read(cx).value().trim();
        if name.is_empty() {
            return;
        }
        let mut settings = self.settings.read(cx).clone();
        let lower_name = name.to_lowercase();
        if settings.pomodoro_labels.is_empty() {
            settings
                .pomodoro_labels
                .push(DEFAULT_POMODORO_LABEL.to_owned());
        }
        if settings
            .pomodoro_labels
            .iter()
            .any(|label| label.to_lowercase() == lower_name)
        {
            self.error = Some("That session name is already in the list.");
        } else {
            settings.pomodoro_labels.push(name.to_owned());
            if let Err(error) = settings.save() {
                eprintln!("Failed to save Pomodoro session name: {error}");
                self.error = Some("Couldn’t save the session name. Try adding it again.");
            } else {
                self.settings.update(cx, |current, cx| {
                    *current = settings;
                    cx.notify();
                });
                self.input.update(cx, TextInput::clear);
            }
        }
        cx.notify();
    }
}

impl Render for PomodoroPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_add = !self.input.read(cx).value().trim().is_empty();
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .on_key_down(cx.listener(|page, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" {
                    page.add(cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(MUTED_TEXT))
                    .child("Add a session name"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().flex_1().min_w(px(0.0)).child(self.input.clone()))
                    .child(
                        div()
                            .id("save-pomodoro-name")
                            .rounded_md()
                            .px(px(16.0))
                            .py(px(9.0))
                            .bg(rgb(if can_add { 0x7941b1 } else { 0x343437 }))
                            .text_color(rgb(if can_add { 0xffffff } else { MUTED_TEXT }))
                            .when(can_add, |button| {
                                button
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0x8d50c9)))
                                    .on_click(cx.listener(|page, _, _, cx| page.add(cx)))
                            })
                            .child("Add"),
                    ),
            )
            .when_some(self.error, |page, error| {
                page.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(0xffa59c))
                        .child(error),
                )
            })
    }
}
