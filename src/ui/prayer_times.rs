use chrono::{DateTime, TimeDelta, Utc};
use gpui::{prelude::*, *};

use crate::prayer::{DaySchedule, PRAYER_NAMES, PRAYERS, PrayerReminder, Schedule};

use super::notch::PAGE_BODY_WIDTH;

const GOLD: u32 = 0xf2b74f;
const TEXT: u32 = 0xf4eee4;
const MUTED: u32 = 0xa7a198;
const TRACK_START: f32 = 8.0;
const TRACK_END: f32 = PAGE_BODY_WIDTH - TRACK_START;
const MARKER_SIZE: f32 = 10.0;

pub(super) fn reminder_banner(reminder: PrayerReminder) -> Div {
    div()
        .max_w(px(248.0))
        .truncate()
        .text_size(px(14.0))
        .text_color(rgb(GOLD))
        .child(reminder.message())
}

pub(super) fn header(schedule: &Schedule, now: DateTime<Utc>) -> Div {
    let local = now.with_timezone(&schedule.city.zone);
    div()
        .max_w(px(264.0))
        .flex()
        .items_center()
        .gap(px(13.0))
        .whitespace_nowrap()
        .text_size(px(12.0))
        .child(
            div()
                .flex_none()
                .relative()
                .left(px(-5.0))
                .child(local.format("%a %d %b %Y").to_string()),
        )
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .text_color(rgb(GOLD))
                .child(schedule.city.name),
        )
        .child(
            div()
                .flex_none()
                .text_color(rgb(0xffffff))
                .text_size(px(15.0))
                .child(local.format("%-I:%M %p").to_string()),
        )
}

pub(super) fn panel(schedule: &Schedule, now: DateTime<Utc>, background: u32) -> Div {
    let local_date = now.with_timezone(&schedule.city.zone).date_naive();
    let active = schedule.active_waqt(now);
    let next = schedule.next(now);
    let shown = active.as_ref().map(|waqt| &waqt.prayer).or(next.as_ref());
    let day = shown.map(|prayer| prayer.day).or_else(|| schedule.today());
    let name = shown.map_or("Unavailable", |prayer| PRAYER_NAMES[prayer.index]);
    let countdown_end = active
        .as_ref()
        .map(|waqt| waqt.end)
        .or_else(|| next.as_ref().map(|prayer| prayer.time));
    let countdown = countdown_end.map_or_else(|| "—".to_owned(), |end| format_countdown(end - now));
    let countdown_label = if active.is_some() {
        "waqt left"
    } else {
        "starts in"
    };
    let tomorrow = day.is_some_and(|day| day.date > local_date);

    let hero = div()
        .h(px(71.0))
        .px(px(TRACK_START))
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .text_color(rgb(TEXT))
                        .text_size(px(if shown.is_some() { 34.0 } else { 22.0 }))
                        .line_height(px(40.0))
                        .child(name),
                )
                .when(tomorrow, |heading| {
                    heading.child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(MUTED))
                            .child("Tomorrow's schedule"),
                    )
                }),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .items_end()
                .child(
                    div()
                        .text_size(px(24.0))
                        .line_height(px(29.0))
                        .text_color(rgb(if active.is_some() { GOLD } else { TEXT }))
                        .child(countdown),
                )
                .when(countdown_end.is_some(), |counter| {
                    counter.child(
                        div()
                            .font_weight(FontWeight::NORMAL)
                            .text_size(px(11.0))
                            .text_color(rgb(MUTED))
                            .child(countdown_label),
                    )
                }),
        );

    div()
        .w(px(PAGE_BODY_WIDTH))
        .flex()
        .flex_col()
        .child(hero)
        .child(timeline(
            schedule,
            day,
            active.as_ref().map(|waqt| waqt.prayer.index),
            now,
            background,
        ))
}

fn timeline(
    schedule: &Schedule,
    day: Option<&DaySchedule>,
    active: Option<usize>,
    now: DateTime<Utc>,
    background: u32,
) -> Div {
    let reached = reached_checkpoints(day, now);
    let fill_end = reached
        .iter()
        .rposition(|&reached| reached)
        .map(prayer_position);
    div()
        .relative()
        .w_full()
        .flex()
        .child(
            div()
                .absolute()
                .top(px(4.0))
                .left(px(TRACK_START))
                .w(px(TRACK_END - TRACK_START))
                .h(px(2.0))
                .bg(rgb(0x393632)),
        )
        .children(PRAYER_NAMES.into_iter().enumerate().map(|(index, name)| {
            let lit = active == Some(index);
            let time = match day {
                Some(day) => day.times[index].map_or_else(
                    || "—".to_owned(),
                    |time| {
                        let local = time.with_timezone(&schedule.city.zone);
                        let suffix = if local.date_naive() > day.date {
                            " +1"
                        } else {
                            ""
                        };
                        format!("{}{suffix}", local.format("%H:%M"))
                    },
                ),
                None => "—".to_owned(),
            };
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .items_center()
                .font_weight(FontWeight::NORMAL)
                .child(
                    div()
                        .size(px(MARKER_SIZE))
                        .mb(px(3.0))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(0x625d55))
                        .bg(rgb(background))
                        .when(reached[index], |marker| {
                            marker.border_color(rgb(GOLD)).bg(rgb(GOLD))
                        })
                        .when(lit, |marker| {
                            marker.shadow(vec![BoxShadow {
                                color: rgba(0xf2b74f99).into(),
                                offset: point(px(0.0), px(0.0)),
                                blur_radius: px(10.0),
                                spread_radius: px(1.0),
                            }])
                        }),
                )
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(12.0))
                        .line_height(px(17.0))
                        .text_color(rgb(if lit { GOLD } else { TEXT }))
                        .child(name),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .line_height(px(14.0))
                        .text_color(rgb(if lit { GOLD } else { MUTED }))
                        .child(time),
                )
        }))
        .when_some(fill_end, |row, end| {
            row.child(
                div()
                    .absolute()
                    .top(px(4.0))
                    .left(px(TRACK_START))
                    .w(px(end - TRACK_START))
                    .h(px(2.0))
                    .bg(rgb(GOLD)),
            )
        })
}

fn prayer_position(index: usize) -> f32 {
    (index as f32 + 0.5) * PAGE_BODY_WIDTH / PRAYER_NAMES.len() as f32
}

fn reached_checkpoints(day: Option<&DaySchedule>, now: DateTime<Utc>) -> [bool; PRAYERS.len()] {
    day.map_or([false; PRAYERS.len()], |day| {
        day.times.map(|time| time.is_some_and(|start| start <= now))
    })
}

fn format_countdown(remaining: TimeDelta) -> String {
    let seconds = remaining.num_seconds() + i64::from(remaining.subsec_nanos() > 0);
    let minutes = (seconds + 59) / 60;
    if seconds < 60 {
        format!("{seconds}s")
    } else if minutes < 60 {
        format!("{minutes}m")
    } else {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::{Schedule, format_countdown, reached_checkpoints};
    use crate::prayer::{DHAKA_ID, city};
    use chrono::{TimeDelta, TimeZone, Utc};

    #[test]
    fn checkpoints_fill_at_their_start_times_and_stay_filled() {
        let noon = Utc.with_ymd_and_hms(2026, 11, 23, 6, 0, 0).unwrap();
        let schedule = Schedule::new(city(DHAKA_ID), noon);
        let today = schedule.today().unwrap();
        for (index, start) in today.times.map(Option::unwrap).into_iter().enumerate() {
            let before = reached_checkpoints(Some(today), start - TimeDelta::seconds(1));
            assert!(!before[index]);
            let reached = reached_checkpoints(Some(today), start);
            assert!(reached[..=index].iter().all(|&filled| filled));
            assert!(reached[index + 1..].iter().all(|&filled| !filled));
        }
    }

    #[test]
    fn countdown_displays_hours_minutes_and_the_final_seconds() {
        assert_eq!(format_countdown(TimeDelta::minutes(62)), "1h 02m");
        assert_eq!(format_countdown(TimeDelta::seconds(61)), "2m");
        assert_eq!(format_countdown(TimeDelta::milliseconds(100)), "1s");
    }
}
