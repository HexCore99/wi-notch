use std::{
    f32::consts::TAU,
    time::{Duration, Instant},
};

use gpui::{prelude::*, *};

use super::{Mode, Notch, PAGE_BODY_WIDTH, Page, SettingsPage};
use crate::{
    appearance::DEFAULT_POMODORO_LABEL,
    icons::{TOMATO_RED, tomato},
    windows::audio::play_system_sound,
};

const ROUNDS: u8 = 4;
const PURPLE: u32 = 0xcf8fec;
const MUTED: u32 = 0xa7a2ae;
const CONTROL: u32 = 0x222126;
const RING_SIZE: f32 = 112.0;
const PICKER_TOP: f32 = 43.0;
const PICKER_HEIGHT: f32 = 22.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Focus,
    ShortBreak,
    LongBreak,
}

impl Phase {
    fn title(self) -> &'static str {
        match self {
            Self::Focus => "Focus",
            Self::ShortBreak => "Short Break",
            Self::LongBreak => "Long Break",
        }
    }

    fn duration(self) -> Duration {
        Duration::from_secs(match self {
            Self::Focus => 25 * 60,
            Self::ShortBreak => 5 * 60,
            Self::LongBreak => 15 * 60,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum State {
    #[default]
    Idle,
    Ready,
    Running(Instant),
    Paused(Duration),
}

#[derive(Clone, Copy)]
enum Action {
    Toggle,
    Stop,
    Reset,
}

#[derive(Default)]
struct Timer {
    phase: Phase,
    state: State,
    completed_rounds: u8,
}

impl Timer {
    fn remaining(&self, now: Instant) -> Duration {
        match self.state {
            State::Running(deadline) => deadline.saturating_duration_since(now),
            State::Paused(remaining) => remaining,
            State::Idle | State::Ready => self.phase.duration(),
        }
    }

    fn round(&self) -> u8 {
        match self.phase {
            Phase::Focus => self.completed_rounds + 1,
            _ => self.completed_rounds,
        }
    }

    fn apply(&mut self, action: Action, now: Instant) {
        match action {
            Action::Toggle => {
                let remaining = self.remaining(now);
                self.state = match self.state {
                    State::Running(_) => State::Paused(remaining),
                    _ => State::Running(now + remaining),
                };
            }
            Action::Stop => *self = Self::default(),
            Action::Reset if self.state != State::Idle => self.state = State::Ready,
            Action::Reset => {}
        }
    }

    fn finish_if_due(&mut self, now: Instant) -> bool {
        if !matches!(self.state, State::Running(deadline) if now >= deadline) {
            return false;
        }
        self.state = State::Ready;
        match self.phase {
            Phase::Focus => {
                self.completed_rounds += 1;
                self.phase = if self.completed_rounds == ROUNDS {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                };
            }
            Phase::ShortBreak => self.phase = Phase::Focus,
            Phase::LongBreak => *self = Self::default(),
        }
        true
    }
}

#[derive(Default)]
pub(super) struct Pomodoro {
    timer: Timer,
    refresh: Option<Task<()>>,
    label: Option<String>,
    menu_open: bool,
}

impl Pomodoro {
    pub(super) fn in_cycle(&self) -> bool {
        self.timer.state != State::Idle
    }
}

impl Notch {
    pub(super) fn close_pomodoro_menu(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.pomodoro.menu_open) {
            cx.notify();
        }
    }

    fn finish_pomodoro_phase(&mut self, now: Instant, cx: &mut Context<Self>) -> bool {
        if !self.pomodoro.timer.finish_if_due(now) {
            return false;
        }
        play_system_sound();
        self.sync_idle_policy(cx);
        cx.notify();
        true
    }

    fn pomodoro_action(&mut self, action: Action, cx: &mut Context<Self>) {
        let now = Instant::now();
        let finished = self.finish_pomodoro_phase(now, cx);
        self.pomodoro.refresh = None;
        if finished && matches!(action, Action::Toggle) {
            return;
        }
        self.pomodoro.timer.apply(action, now);
        if let State::Running(deadline) = self.pomodoro.timer.state {
            self.pomodoro.refresh = Some(cx.spawn(async move |this, cx| {
                loop {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if !remaining.is_zero() {
                        // Wake when the next displayed second begins or the phase ends.
                        let next_tick =
                            remaining - Duration::from_secs(whole_seconds(remaining) - 1);
                        cx.background_executor().timer(next_tick).await;
                    }
                    let running = this.update(cx, |notch, cx| {
                        if notch.finish_pomodoro_phase(Instant::now(), cx) {
                            return false;
                        }
                        if matches!(notch.mode, Mode::Compact | Mode::Expanded(Page::Pomodoro)) {
                            cx.notify();
                        }
                        true
                    });
                    if !matches!(running, Ok(true)) {
                        break;
                    }
                }
            }));
        }
        self.sync_idle_policy(cx);
        cx.notify();
    }

    pub(super) fn compact_pomodoro(&self, now: Instant) -> Div {
        let timer = &self.pomodoro.timer;
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(tomato(15.0))
            .child(
                div()
                    .text_size(px(15.0))
                    .text_color(rgb(if matches!(timer.state, State::Running(_)) {
                        0xffffff
                    } else {
                        0xd2b7dd
                    }))
                    .child(countdown(timer.remaining(now))),
            )
    }

    pub(super) fn pomodoro_panel(&self, now: Instant, cx: &Context<Self>) -> Div {
        let timer = &self.pomodoro.timer;
        let remaining = timer.remaining(now);
        let duration = timer.phase.duration();
        let elapsed = duration.saturating_sub(remaining);
        let progress = elapsed.as_secs() as f32 / duration.as_secs_f32();
        let (symbol, action) = match timer.state {
            State::Running(_) => ("Ⅱ", "Pause"),
            State::Paused(_) => ("▶", "Resume"),
            State::Ready | State::Idle => ("▶", "Start"),
        };
        div()
            .w(px(PAGE_BODY_WIDTH))
            .h(px(127.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .relative()
                    .size(px(RING_SIZE))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(progress_ring(progress))
                    .child(tomato(16.0))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .line_height(px(14.0))
                            .text_color(rgb(MUTED))
                            .child(timer.phase.title()),
                    )
                    .child(
                        div()
                            .text_size(px(26.0))
                            .line_height(px(28.0))
                            .child(countdown(remaining)),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .px(px(6.0))
                            .rounded_full()
                            .bg(rgb(0x352641))
                            .text_color(rgb(0xebc9fa))
                            .text_size(px(9.0))
                            .line_height(px(15.0))
                            .child(format!("Pomodoro {}/{}", timer.round(), ROUNDS)),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w(px(0.0))
                    .h(px(116.0))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .text_size(px(20.0))
                            .line_height(px(22.0))
                            .child(tomato(17.0))
                            .child("Pomodoro"),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .line_height(px(14.0))
                            .text_color(rgb(MUTED))
                            .child("Focus · Work · Rest · Repeat"),
                    )
                    .child(
                        div()
                            .mt(px(7.0))
                            .h(px(PICKER_HEIGHT))
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .children((1..=ROUNDS).map(|round| {
                                let color = if round <= timer.completed_rounds {
                                    0x8e60a8
                                } else if round == timer.round() {
                                    PURPLE
                                } else {
                                    0x68656d
                                };
                                div()
                                    .size(px(8.0))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(rgb(color))
                                    .when(round <= timer.round(), |dot| dot.bg(rgb(color)))
                            })),
                    )
                    .child(
                        div()
                            .mt(px(9.0))
                            .flex()
                            .gap(px(7.0))
                            .child(
                                timer_button("pomodoro-start-pause")
                                    .flex_1()
                                    .gap(px(6.0))
                                    .bg(rgb(0x8144b8))
                                    .cursor_pointer()
                                    .hover(|button| button.bg(rgb(0x9657cc)))
                                    .text_size(px(13.0))
                                    .child(div().text_size(px(10.0)).child(symbol))
                                    .child(action)
                                    .on_click(cx.listener(|notch, _, _, cx| {
                                        notch.pomodoro_action(Action::Toggle, cx);
                                    })),
                            )
                            .child(
                                secondary_button("pomodoro-stop", self.pomodoro.in_cycle())
                                    .child(div().size(px(9.0)).rounded_sm().bg(rgb(0xffffff)))
                                    .on_click(cx.listener(|notch, _, _, cx| {
                                        if notch.pomodoro.in_cycle() {
                                            notch.pomodoro_action(Action::Stop, cx);
                                        }
                                    })),
                            )
                            .child(
                                secondary_button("pomodoro-reset", self.pomodoro.in_cycle())
                                    .text_size(px(19.0))
                                    .child("↺")
                                    .on_click(cx.listener(|notch, _, _, cx| {
                                        if notch.pomodoro.in_cycle() {
                                            notch.pomodoro_action(Action::Reset, cx);
                                        }
                                    })),
                            ),
                    )
                    .child(self.pomodoro_picker(cx)),
            )
    }

    fn pomodoro_picker(&self, cx: &Context<Self>) -> Stateful<Div> {
        let names = &self.appearance.read(cx).pomodoro_labels;
        let default = names.first().map_or(DEFAULT_POMODORO_LABEL, String::as_str);
        let selected = self.pomodoro.label.as_deref().unwrap_or(default);
        div()
            .id("pomodoro-picker")
            .absolute()
            .top(px(PICKER_TOP))
            .right(px(0.0))
            .w(px(116.0))
            .flex()
            .flex_col()
            .items_end()
            .child(
                div()
                    .id("pomodoro-picker-toggle")
                    .w(px(116.0))
                    .h(px(PICKER_HEIGHT))
                    .px(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0x45424c))
                    .bg(rgb(CONTROL))
                    .hover(|button| button.bg(rgb(0x302e35)))
                    .cursor_pointer()
                    .text_size(px(10.0))
                    .child(div().text_color(rgb(MUTED)).child("☷"))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .truncate()
                            .child(selected.to_owned()),
                    )
                    .child(div().text_color(rgb(MUTED)).child("⌄"))
                    .on_click(cx.listener(|notch, _, _, cx| {
                        notch.pomodoro.menu_open = !notch.pomodoro.menu_open;
                        cx.notify();
                    })),
            )
            .when(self.pomodoro.menu_open, |picker| {
                picker
                    .w_full()
                    .bottom(px(0.0))
                    .on_mouse_down_out(cx.listener(|notch, _, _, cx| {
                        notch.close_pomodoro_menu(cx);
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.0))
                            .w_full()
                            .flex()
                            .flex_col()
                            .rounded_md()
                            .overflow_hidden()
                            .occlude()
                            .bg(rgb(CONTROL))
                            .border_1()
                            .border_color(rgb(0x45424c))
                            .child(
                                div()
                                    .id("pomodoro-picker-names")
                                    .flex_1()
                                    .min_h(px(0.0))
                                    .overflow_y_scroll()
                                    .children(
                                        names
                                            .iter()
                                            .map(String::as_str)
                                            .chain(names.is_empty().then_some(default))
                                            .enumerate()
                                            .map(|(index, name)| {
                                                let name = name.to_owned();
                                                picker_row(index, &name)
                                                    .when(name == selected, |row| {
                                                        row.text_color(rgb(PURPLE))
                                                    })
                                                    .on_click(cx.listener(
                                                        move |notch, _, _, cx| {
                                                            notch.pomodoro.label =
                                                                Some(name.clone());
                                                            notch.close_pomodoro_menu(cx);
                                                        },
                                                    ))
                                            }),
                                    ),
                            )
                            .child(
                                picker_row("add-pomodoro-name", "+ Add")
                                    .border_t_1()
                                    .border_color(rgb(0x45424c))
                                    .on_click(cx.listener(|notch, _, _, cx| {
                                        notch.close_pomodoro_menu(cx);
                                        notch.open_settings_page(Some(SettingsPage::Pomodoro), cx);
                                    })),
                            ),
                    )
            })
    }
}

fn timer_button(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
}

fn secondary_button(id: &'static str, enabled: bool) -> Stateful<Div> {
    timer_button(id)
        .w(px(28.0))
        .bg(rgb(CONTROL))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x36343c)))
        })
        .when(!enabled, |button| button.opacity(0.45))
}

fn picker_row(id: impl Into<ElementId>, label: &str) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(16.0))
        .px(px(7.0))
        .flex_none()
        .flex()
        .items_center()
        .text_size(px(10.0))
        .line_height(px(13.0))
        .hover(|style| style.bg(rgb(0x36343c)))
        .cursor_pointer()
        .child(div().min_w(px(0.0)).truncate().child(label.to_owned()))
}

fn progress_ring(progress: f32) -> impl IntoElement {
    const STROKE: f32 = 5.0;
    const MARKER_SIZE: f32 = 10.0;
    div()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .size_full()
        .child(
            div()
                .size_full()
                .rounded_full()
                .border(px(STROKE))
                .border_color(rgb(if progress >= 1.0 {
                    TOMATO_RED
                } else {
                    0x383541
                })),
        )
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, _| {
                    let center = bounds.center();
                    let radius = px((RING_SIZE - STROKE) / 2.0);
                    let angle = progress * TAU;
                    let start = point(center.x, center.y - radius);
                    let marker = point(
                        center.x + radius * angle.sin(),
                        center.y - radius * angle.cos(),
                    );
                    let color = rgb(TOMATO_RED);
                    if progress > 0.0 && progress < 1.0 {
                        let mut arc = PathBuilder::stroke(px(STROKE));
                        arc.move_to(start);
                        arc.arc_to(point(radius, radius), px(0.0), progress > 0.5, true, marker);
                        if let Ok(path) = arc.build() {
                            window.paint_path(path, color);
                        }
                    }
                    for (center, diameter) in [(start, STROKE), (marker, MARKER_SIZE)] {
                        let radius = px(diameter / 2.0);
                        let bounds = Bounds::new(
                            center - point(radius, radius),
                            size(px(diameter), px(diameter)),
                        );
                        window.paint_quad(fill(bounds, color).corner_radii(radius));
                    }
                },
            )
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .size_full(),
        )
}

fn whole_seconds(remaining: Duration) -> u64 {
    remaining.as_secs() + u64::from(remaining.subsec_nanos() != 0)
}

fn countdown(remaining: Duration) -> String {
    let seconds = whole_seconds(remaining);
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::{Action, Duration, Instant, Phase, ROUNDS, State, Timer, countdown};

    #[test]
    fn pause_and_resume_keep_the_remaining_time() {
        let start = Instant::now();
        let mut timer = Timer::default();
        timer.apply(Action::Toggle, start);
        let paused = start + Duration::from_millis(1200);
        timer.apply(Action::Toggle, paused);
        let resumed = start + Duration::from_secs(120);
        assert_eq!(countdown(timer.remaining(resumed)), "24:59");
        timer.apply(Action::Toggle, resumed);
        assert_eq!(
            countdown(timer.remaining(resumed + Duration::from_secs(2))),
            "24:57"
        );
    }

    #[test]
    fn phases_advance_once_and_each_waits_for_start() {
        let mut now = Instant::now();
        let mut timer = Timer::default();
        for round in 1..=ROUNDS {
            assert_eq!(timer.phase, Phase::Focus);
            assert_eq!(timer.round(), round);
            timer.apply(Action::Toggle, now);
            now += Duration::from_secs(25 * 60);
            assert!(!timer.finish_if_due(now - Duration::from_millis(1)));
            assert!(timer.finish_if_due(now));
            assert_eq!(timer.round(), round);
            assert_eq!(timer.state, State::Ready);
            assert!(!timer.finish_if_due(now + Duration::from_secs(3600)));
            let expected_break = if round == ROUNDS {
                Phase::LongBreak
            } else {
                Phase::ShortBreak
            };
            assert_eq!(timer.phase, expected_break);
            assert_eq!(
                countdown(timer.remaining(now)),
                if round == ROUNDS { "15:00" } else { "05:00" }
            );
            timer.apply(Action::Toggle, now);
            now += expected_break.duration();
            assert!(timer.finish_if_due(now));
        }
        assert_eq!(timer.state, State::Idle);
        assert_eq!(timer.round(), 1);
    }

    #[test]
    fn reset_keeps_the_phase_but_stop_ends_the_cycle() {
        let start = Instant::now();
        let mut timer = Timer::default();
        timer.apply(Action::Toggle, start);
        let break_start = start + Phase::Focus.duration();
        assert!(timer.finish_if_due(break_start));
        timer.apply(Action::Toggle, break_start);
        let later = break_start + Duration::from_secs(10);
        timer.apply(Action::Reset, later);
        assert_eq!(timer.state, State::Ready);
        assert_eq!(timer.phase, Phase::ShortBreak);
        assert_eq!(countdown(timer.remaining(later)), "05:00");
        timer.apply(Action::Stop, later);
        assert_eq!(timer.state, State::Idle);
        assert_eq!(timer.phase, Phase::Focus);
        assert_eq!(timer.round(), 1);
    }
}
