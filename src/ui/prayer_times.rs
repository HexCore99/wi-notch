use chrono::{DateTime, TimeDelta, Utc};
use gpui::{prelude::*, *};

use crate::prayer::{DaySchedule, PRAYERS, Schedule, ScheduledPrayer};

use super::notch::PAGE_BODY_WIDTH;

const GOLD: u32 = 0xf2b74f;
const TEXT: u32 = 0xf4eee4;
const MUTED: u32 = 0xa7a198;
const TRACK_START: f32 = 8.0;
const TRACK_END: f32 = PAGE_BODY_WIDTH - TRACK_START;
const MARKER_SIZE: f32 = 10.0;
const PRAYER_NAMES: [&str; PRAYERS.len()] = ["Fajr", "Dhuhr", "Asr", "Maghrib", "Isha"];

pub(super) fn header(schedule: &Schedule, now: DateTime<Utc>) -> Div {
    let local = now.with_timezone(&schedule.city.zone);
    div()
        .w(px(248.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .font_family("Consolas")
        .font_weight(FontWeight::NORMAL)
        .text_size(px(11.0))
        .text_color(rgb(MUTED))
        .child(
            div()
                .flex_none()
                .child(local.format("%a %d %b").to_string().to_uppercase()),
        )
        .child(
            div()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap(px(5.0))
                .child(div().truncate().child(schedule.city.name.to_uppercase()))
                .child(
                    div()
                        .flex_none()
                        .child(format!("· {}", local.format("%-I:%M %p"))),
                ),
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
    let day_label = day.and_then(|day| match day.date.cmp(&local_date) {
        std::cmp::Ordering::Greater => Some("Tomorrow's schedule"),
        std::cmp::Ordering::Less => Some("Yesterday's schedule"),
        std::cmp::Ordering::Equal => None,
    });

    let previous = active
        .as_ref()
        .map(|waqt| waqt.prayer)
        .or_else(|| schedule.previous(now));
    let progress = day.and_then(|day| {
        Some(progress_position(
            day,
            previous.as_ref()?,
            next.as_ref()?,
            now,
        ))
    });

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
                        .font_family("Georgia")
                        .font_weight(FontWeight::NORMAL)
                        .text_color(rgb(TEXT))
                        .text_size(px(if shown.is_some() { 38.0 } else { 22.0 }))
                        .line_height(px(43.0))
                        .child(name),
                )
                .when_some(day_label, |heading, label| {
                    heading.child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(MUTED))
                            .child(label),
                    )
                }),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .items_end()
                .font_family("Consolas")
                .child(
                    div()
                        .text_size(px(26.0))
                        .line_height(px(30.0))
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
            progress,
            background,
        ))
}

fn timeline(
    schedule: &Schedule,
    day: Option<&DaySchedule>,
    active: Option<usize>,
    progress: Option<f32>,
    background: u32,
) -> Div {
    let progress_color = if active.is_some() { GOLD } else { MUTED };
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
        .when_some(progress, |row, position| {
            row.child(
                div()
                    .absolute()
                    .top(px(4.0))
                    .left(px(TRACK_START))
                    .w(px(position - TRACK_START))
                    .h(px(2.0))
                    .bg(rgb(progress_color)),
            )
        })
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
                        .when(lit, |marker| {
                            marker
                                .border_color(rgb(GOLD))
                                .bg(rgb(GOLD))
                                .shadow(vec![BoxShadow {
                                    color: rgba(0xf2b74f99).into(),
                                    offset: point(px(0.0), px(0.0)),
                                    blur_radius: px(10.0),
                                    spread_radius: px(1.0),
                                }])
                        }),
                )
                .child(
                    div()
                        .font_family("Georgia")
                        .text_size(px(14.0))
                        .line_height(px(17.0))
                        .text_color(rgb(if lit { GOLD } else { TEXT }))
                        .child(name),
                )
                .child(
                    div()
                        .font_family("Consolas")
                        .text_size(px(11.0))
                        .line_height(px(14.0))
                        .text_color(rgb(if lit { GOLD } else { MUTED }))
                        .child(time),
                )
        }))
        .when_some(progress, |row, position| {
            let dot = 6.0;
            row.child(
                div()
                    .absolute()
                    .top(px((MARKER_SIZE - dot) / 2.0))
                    .left(px(position - dot / 2.0))
                    .size(px(dot))
                    .rounded_full()
                    .bg(rgb(if active.is_some() { 0xffedc6 } else { MUTED })),
            )
        })
}

fn prayer_position(index: usize) -> f32 {
    (index as f32 + 0.5) * PAGE_BODY_WIDTH / PRAYER_NAMES.len() as f32
}

fn progress_position(
    day: &DaySchedule,
    previous: &ScheduledPrayer,
    next: &ScheduledPrayer,
    now: DateTime<Utc>,
) -> f32 {
    // Overnight the visible schedule can show either side of the Isha to Fajr interval.
    let start = if previous.day.date == day.date {
        prayer_position(previous.index)
    } else {
        TRACK_START
    };
    let end = if next.day.date == day.date {
        prayer_position(next.index)
    } else {
        TRACK_END
    };
    let elapsed = (now - previous.time).as_seconds_f64();
    let interval = (next.time - previous.time).as_seconds_f64();
    start + (end - start) * (elapsed / interval) as f32
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
    use super::{Schedule, TRACK_END, format_countdown, prayer_position, progress_position};
    use crate::prayer::{DHAKA_ID, city};
    use chrono::{TimeDelta, TimeZone, Utc};

    #[test]
    fn progress_tracks_elapsed_time_and_extends_past_isha() {
        let noon = Utc.with_ymd_and_hms(2026, 11, 23, 6, 0, 0).unwrap();
        let schedule = Schedule::new(city(DHAKA_ID), noon);
        let today = schedule.today().unwrap();
        let [fajr, dhuhr, _, _, isha] = today.times.map(Option::unwrap);
        let next_fajr = schedule.next(isha).unwrap().time;

        for (start, end, left, right) in [
            (fajr, dhuhr, prayer_position(0), prayer_position(1)),
            (isha, next_fajr, prayer_position(4), TRACK_END),
        ] {
            let half = start + (end - start) / 2;
            let position = |now| {
                progress_position(
                    today,
                    &schedule.previous(now).unwrap(),
                    &schedule.next(now).unwrap(),
                    now,
                )
            };
            assert!((position(start) - left).abs() < 0.001);
            assert!((position(half) - (left + right) / 2.0).abs() < 0.001);
        }
    }

    #[test]
    fn countdown_displays_hours_minutes_and_the_final_seconds() {
        assert_eq!(format_countdown(TimeDelta::minutes(62)), "1h 02m");
        assert_eq!(format_countdown(TimeDelta::seconds(61)), "2m");
        assert_eq!(format_countdown(TimeDelta::milliseconds(100)), "1s");
    }
}
