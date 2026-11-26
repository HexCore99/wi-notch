use gpui::{prelude::*, *};

use super::{
    CONTROL_BACKGROUND, CONTROL_HOVER, MUTED_TEXT, setting_group, setting_row, setting_switch,
    text_input::{TextChanged, TextInput},
};
use crate::appearance::AppearanceSettings;

#[derive(Clone)]
struct SessionDrag {
    index: usize,
    name: SharedString,
}

impl Render for SessionDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(240.0))
            .px(px(10.0))
            .py(px(5.0))
            .rounded_md()
            .bg(rgb(CONTROL_BACKGROUND))
            .text_color(rgb(0xffffff))
            .text_size(px(13.0))
            .truncate()
            .child(self.name.clone())
    }
}

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
        if settings
            .pomodoro_session_names
            .iter()
            .any(|label| label.to_lowercase() == lower_name)
        {
            self.error = Some("That session name is already in the list.");
            cx.notify();
            return;
        }

        settings.pomodoro_session_names.push(name.to_owned());
        if self.save_settings(settings, cx) {
            self.input.update(cx, TextInput::clear);
        }
    }

    fn remove(&mut self, index: usize, cx: &mut Context<Self>) {
        let mut settings = self.settings.read(cx).clone();
        let names = &mut settings.pomodoro_session_names;
        if index < names.len() {
            names.remove(index);
            self.save_settings(settings, cx);
        }
    }

    fn reorder(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let mut settings = self.settings.read(cx).clone();
        let names = &mut settings.pomodoro_session_names;
        if from == to || from >= names.len() || to >= names.len() {
            return;
        }
        let name = names.remove(from);
        names.insert(to, name);
        self.save_settings(settings, cx);
    }

    fn toggle_repeat_sound(&mut self, cx: &mut Context<Self>) {
        let mut settings = self.settings.read(cx).clone();
        settings.pomodoro_repeat_sound = !settings.pomodoro_repeat_sound;
        self.save_settings(settings, cx);
    }

    fn save_settings(&mut self, settings: AppearanceSettings, cx: &mut Context<Self>) -> bool {
        if let Err(error) = settings.save() {
            eprintln!("Failed to save Pomodoro settings: {error}");
            self.error = Some("Couldn’t save Pomodoro settings. Try again.");
            cx.notify();
            return false;
        }

        self.error = None;
        self.settings.update(cx, |current, cx| {
            *current = settings;
            cx.notify();
        });
        cx.notify();
        true
    }

    fn session_row(index: usize, name: &str, cx: &Context<Self>) -> Stateful<Div> {
        let name = SharedString::from(name.to_owned());
        div()
            .id(("session", index))
            .h(px(38.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .pl(px(12.0))
            .pr(px(8.0))
            .when(index > 0, |row| {
                row.border_t_1().border_color(rgb(0x303032))
            })
            .drag_over::<SessionDrag>(move |style, session, _, _| {
                if session.index == index {
                    style
                } else {
                    style.bg(rgb(CONTROL_BACKGROUND))
                }
            })
            .on_drop(cx.listener(move |page, session: &SessionDrag, _, cx| {
                page.reorder(session.index, index, cx);
            }))
            .child(div().flex_1().min_w(px(0.0)).truncate().child(name.clone()))
            .child(
                session_control("drag-session")
                    .cursor(CursorStyle::OpenHand)
                    .on_drag(SessionDrag { index, name }, |session, _, _, cx| {
                        cx.new(|_| session.clone())
                    })
                    .child("≡"),
            )
            .child(
                session_control("remove-session")
                    .cursor_pointer()
                    .on_click(cx.listener(move |page, _, _, cx| page.remove(index, cx)))
                    .child("−"),
            )
    }
}

fn session_control(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(24.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .text_size(px(16.0))
        .text_color(rgb(MUTED_TEXT))
        .hover(|button| button.bg(rgb(CONTROL_HOVER)).text_color(rgb(0xffffff)))
}

impl Render for PomodoroPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_add = !self.input.read(cx).value().trim().is_empty();
        let settings = self.settings.read(cx);
        let names = &settings.pomodoro_session_names;
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
            .child(setting_group(
                "Reminders",
                [setting_row(
                    "Repeat sound every 30 seconds",
                    setting_switch("pomodoro-repeat-sound", settings.pomodoro_repeat_sound)
                        .on_click(cx.listener(|page, _, _, cx| page.toggle_repeat_sound(cx))),
                )],
            ))
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
            .child(
                div()
                    .mt(px(10.0))
                    .text_size(px(12.0))
                    .text_color(rgb(MUTED_TEXT))
                    .child("Saved sessions"),
            )
            .child(
                div()
                    .id("saved-pomodoro-sessions")
                    .max_h(px(280.0))
                    .overflow_y_scroll()
                    .rounded_md()
                    .bg(rgb(0x28282a))
                    .when(names.is_empty(), |list| {
                        list.child(
                            div()
                                .px(px(12.0))
                                .py(px(8.0))
                                .text_color(rgb(MUTED_TEXT))
                                .child("No saved sessions"),
                        )
                    })
                    .children(
                        names
                            .iter()
                            .enumerate()
                            .map(|(index, name)| Self::session_row(index, name, cx)),
                    ),
            )
    }
}
