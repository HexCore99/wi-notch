use chrono::{DateTime, Utc};
use gpui::{prelude::*, *};

use crate::prayer::{PRAYERS, Schedule, ScheduledPrayer};

use super::notch::PAGE_BODY_WIDTH;

const ACCENT: u32 = 0xba9cff;
const MUTED: u32 = 0xa5a2b1;
const TRACK_START: f32 = 4.0;
const TRACK_HEIGHT: f32 = 8.0;
const PROGRESS_DOT: f32 = 7.0;
const PRAYER_LABELS: [(&str, &str); PRAYERS.len()] = [
    ("Fajr", "⛅\u{fe0e}"),
    ("Dhuhr", "☀\u{fe0e}"),
    ("Asr", "☀\u{fe0e}"),
    ("Maghrib", "◒"),
    ("Isha", "☾"),
];

fn prayer_position(index: usize) -> f32 {
    (index as f32 + 0.5) * PAGE_BODY_WIDTH / PRAYER_LABELS.len() as f32
}

fn progress_position(
    schedule: &Schedule,
    now: DateTime<Utc>,
    next: &ScheduledPrayer,
) -> Option<f32> {
    let previous = schedule.previous(now)?;
    // Overnight the incoming segment ends at the first prayer on the new schedule.
    let start = if previous.day.date == next.day.date {
        prayer_position(previous.index)
    } else {
        TRACK_START
    };
    let elapsed = (now - previous.time).as_seconds_f64();
    let interval = (next.time - previous.time).as_seconds_f64();
    Some(start + (prayer_position(next.index) - start) * (elapsed / interval) as f32)
}

pub(super) fn header(schedule: &Schedule, now: DateTime<Utc>) -> Div {
    let local = now.with_timezone(&schedule.city.zone);
    div()
        .w(px(248.0))
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgb(ACCENT))
                .child("Prayer Times"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_size(px(11.0))
                .child(
                    div()
                        .text_color(rgb(MUTED))
                        .child(local.format("%a %d %b").to_string()),
                )
                .child(local.format("%-I:%M %p").to_string()),
        )
}

pub(super) fn panel(schedule: &Schedule, now: DateTime<Utc>, background: u32) -> Div {
    let local_date = now.with_timezone(&schedule.city.zone).date_naive();
    let next = schedule.next(now);
    let progress = next
        .as_ref()
        .and_then(|next| progress_position(schedule, now, next));
    let day = next
        .as_ref()
        .map(|prayer| prayer.day)
        .or_else(|| schedule.today());
    let day_label = day.and_then(|day| match day.date.cmp(&local_date) {
        std::cmp::Ordering::Greater => Some("Tomorrow"),
        std::cmp::Ordering::Less => Some("Yesterday"),
        std::cmp::Ordering::Equal => None,
    });
    let next_name = next
        .as_ref()
        .map_or("Unavailable", |prayer| PRAYER_LABELS[prayer.index].0);
    let countdown = next.as_ref().map_or_else(
        || "—".to_owned(),
        |prayer| {
            let remaining = prayer.time - now;
            let seconds = remaining.num_seconds() + i64::from(remaining.subsec_nanos() > 0);
            format!(
                "{:02}:{:02}:{:02}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            )
        },
    );
    let missing_times = day.is_none_or(|day| day.times.iter().any(Option::is_none));
    let next_prayer = div()
        .h(px(52.0))
        .flex()
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .pl(px(6.0))
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(rgb(MUTED))
                        .child("Next prayer"),
                )
                .child(div().truncate().text_size(px(23.0)).child(next_name)),
        )
        .child(
            div()
                .w(px(142.0))
                .pl(px(14.0))
                .border_l_1()
                .border_color(rgba(0xffffff28))
                .flex()
                .flex_col()
                .child(
                    div()
                        .font_family("Consolas")
                        .text_size(px(25.0))
                        .text_color(rgb(ACCENT))
                        .child(countdown),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(rgb(MUTED))
                        .child("Remaining"),
                ),
        );

    let timeline = div()
        .relative()
        .w_full()
        .pt(px(TRACK_HEIGHT))
        .flex()
        .child(
            div()
                .absolute()
                .top(px(3.0))
                .left(px(TRACK_START))
                .w(px(prayer_position(PRAYER_LABELS.len() - 1) - TRACK_START))
                .h(px(1.0))
                .bg(rgba(0xba9cff50)),
        )
        .when_some(progress, |timeline, position| {
            timeline
                .child(
                    div()
                        .absolute()
                        .top(px(2.5))
                        .left(px(TRACK_START))
                        .w(px(position - TRACK_START))
                        .h(px(2.0))
                        .rounded_full()
                        .bg(rgb(ACCENT)),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(0.0))
                        .left(px(position - PROGRESS_DOT / 2.0))
                        .size(px(PROGRESS_DOT))
                        .rounded_full()
                        .bg(rgb(ACCENT)),
                )
        })
        .children(
            PRAYER_LABELS
                .into_iter()
                .enumerate()
                .map(|(index, (label, icon))| {
                    let active = next.as_ref().is_some_and(|prayer| prayer.index == index);
                    let time = day.and_then(|day| day.times[index]).map_or_else(
                        || "—".to_owned(),
                        |time| {
                            let local_time = time.with_timezone(&schedule.city.zone);
                            let suffix =
                                if day.is_some_and(|day| local_time.date_naive() > day.date) {
                                    " +1"
                                } else {
                                    ""
                                };
                            format!("{}{suffix}", local_time.format("%H:%M"))
                        },
                    );
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex()
                        .flex_col()
                        .items_center()
                        .text_size(px(11.0))
                        .line_height(px(14.0))
                        .when(active, |item| item.text_color(rgb(ACCENT)))
                        .child(
                            div()
                                .size(px(19.0))
                                .rounded_full()
                                .border_1()
                                .border_color(if active {
                                    rgb(ACCENT).into()
                                } else {
                                    rgba(0xba9cff55)
                                })
                                .bg(rgb(background))
                                .flex()
                                .items_center()
                                .justify_center()
                                .font_family("Segoe UI Symbol")
                                .text_size(px(13.0))
                                .child(icon),
                        )
                        .child(label)
                        .child(
                            div()
                                .when(!active, |time| time.text_color(rgb(MUTED)))
                                .child(time),
                        )
                }),
        );

    div()
        .w(px(PAGE_BODY_WIDTH))
        .flex()
        .flex_col()
        .child(next_prayer)
        .child(
            div()
                .h(px(14.0))
                .px(px(6.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_size(px(10.0))
                .text_color(rgb(MUTED))
                .child(div().truncate().child(schedule.city.name))
                .when(missing_times, |line| line.child("Some times unavailable"))
                .when_some(day_label, |line, label| {
                    line.child(
                        div()
                            .ml_auto()
                            .text_color(rgb(ACCENT))
                            .flex_none()
                            .child(label),
                    )
                }),
        )
        .child(timeline)
}

#[cfg(test)]
mod tests {
    use super::{Schedule, TRACK_START, prayer_position, progress_position};
    use crate::prayer::{DHAKA_ID, city};
    use chrono::{TimeZone, Utc};

    #[test]
    fn timeline_dot_moves_between_prayers_and_wraps_for_fajr() {
        let noon = Utc.with_ymd_and_hms(2026, 11, 23, 6, 0, 0).unwrap();
        let schedule = Schedule::new(city(DHAKA_ID), noon);
        let today = schedule.today().unwrap();
        let [fajr, dhuhr, _, _, isha] = today.times.map(Option::unwrap);
        let next_fajr = schedule.next(isha).unwrap().time;

        for (start, end, left, right) in [
            (fajr, dhuhr, prayer_position(0), prayer_position(1)),
            (isha, next_fajr, TRACK_START, prayer_position(0)),
        ] {
            let half = start + (end - start) / 2;
            let position =
                |now| progress_position(&schedule, now, &schedule.next(now).unwrap()).unwrap();
            assert!((position(start) - left).abs() < 0.001);
            assert!((position(half) - (left + right) / 2.0).abs() < 0.001);
        }
    }
}
