use adhan::Prayer;
use chrono::{DateTime, TimeDelta, Utc};
use gpui::{prelude::*, *};

use crate::prayer::{
    DaySchedule, PRAYER_NAMES, PRAYERS, PrayerReminder, Schedule, ScheduledPrayer, Waqt,
};

use super::notch::PAGE_BODY_WIDTH;

const GOLD: u32 = 0xf2b74f;
const MINT: u32 = 0x79ead8;
const TEXT: u32 = 0xf4eee4;
const MUTED: u32 = 0xa7a198;
const HERO_PADDING: f32 = 8.0;
const TRACK_END: f32 = PAGE_BODY_WIDTH - 4.0;
const MARKER_SIZE: f32 = 10.0;

pub(super) fn reminder_banner(reminder: PrayerReminder) -> Div {
    div()
        .max_w(px(248.0))
        .truncate()
        .text_size(px(14.0))
        .text_color(rgb(GOLD))
        .child(reminder.message())
}

pub(super) fn header(
    schedule: &Schedule,
    now: DateTime<Utc>,
    date: impl IntoElement,
    reveal: f32,
) -> Div {
    let local = now.with_timezone(&schedule.city.zone);
    div()
        .max_w(px(264.0))
        .flex()
        .items_center()
        .gap(px(13.0 * reveal))
        .whitespace_nowrap()
        .text_size(px(12.0))
        .child(
            div()
                .flex()
                .items_center()
                .min_w(px(0.0))
                .max_w(px(264.0 * reveal))
                .gap(px(13.0))
                .overflow_hidden()
                .opacity(reveal)
                .child(div().flex_none().child(date))
                .child(
                    div()
                        .min_w(px(0.0))
                        .truncate()
                        .text_color(rgb(GOLD))
                        .child(schedule.city.name),
                ),
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
        .px(px(HERO_PADDING))
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
            active.as_ref(),
            next,
            now,
            background,
        ))
}

fn timeline(
    schedule: &Schedule,
    day: Option<&DaySchedule>,
    active: Option<&Waqt<'_>>,
    next: Option<ScheduledPrayer<'_>>,
    now: DateTime<Utc>,
    background: u32,
) -> Div {
    let reached = reached_checkpoints(day, now);
    let track_start = prayer_position(0);
    let position = active
        .map(|waqt| waqt.prayer)
        .or_else(|| schedule.previous(now))
        .filter(|prayer| day.is_some_and(|day| day.date == prayer.day.date))
        .map(|prayer| progress_position(prayer, next, now));
    let fill_end = position.or_else(|| {
        reached
            .iter()
            .rposition(|&reached| reached)
            .map(prayer_position)
    });
    div()
        .relative()
        .w_full()
        .flex()
        .child(
            div()
                .absolute()
                .top(px(4.0))
                .left(px(track_start))
                .w(px(TRACK_END - track_start))
                .h(px(2.0))
                .bg(rgb(0x393632)),
        )
        .when_some(fill_end, |row, end| {
            row.child(
                div()
                    .absolute()
                    .top(px(4.0))
                    .left(px(track_start))
                    .w(px(end - track_start))
                    .h(px(2.0))
                    .bg(rgb(GOLD)),
            )
        })
        .children(PRAYER_NAMES.into_iter().enumerate().map(|(index, name)| {
            let lit = active.is_some_and(|waqt| waqt.prayer.index == index);
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
        .when_some(position, |row, position| {
            row.child(progress_marker(position))
        })
}

fn progress_marker(position: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let center = point(bounds.origin.x + px(position), bounds.center().y);
            let at = |x, y| center + point(px(x), px(y));
            let tail = 8.0_f32.min(position - prayer_position(0) + MARKER_SIZE / 2.0);
            let glow_bounds = Bounds::new(at(-3.5, -3.5), size(px(7.0), px(7.0)));
            window.paint_shadows(
                glow_bounds,
                px(3.5).into(),
                &[BoxShadow {
                    color: rgba(0x79ead870).into(),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(7.0),
                    spread_radius: px(0.5),
                }],
            );

            let mut drop = PathBuilder::fill();
            drop.move_to(at(-tail, 0.0));
            drop.cubic_bezier_to(at(0.0, -3.5), at(-tail * 0.65, -0.5), at(-2.8, -3.5));
            drop.cubic_bezier_to(at(3.5, 0.0), at(1.9, -3.5), at(3.5, -1.9));
            drop.cubic_bezier_to(at(0.0, 3.5), at(3.5, 1.9), at(1.9, 3.5));
            drop.cubic_bezier_to(at(-tail, 0.0), at(-2.8, 3.5), at(-tail * 0.65, 0.5));
            drop.close();
            if let Ok(path) = drop.build() {
                window.paint_path(path, rgb(MINT));
            }
        },
    )
    .absolute()
    .top(px(0.0))
    .left(px(0.0))
    .w_full()
    .h(px(MARKER_SIZE))
}

fn prayer_position(index: usize) -> f32 {
    (index as f32 + 0.5) * PAGE_BODY_WIDTH / PRAYER_NAMES.len() as f32
}

fn progress_position(
    prayer: ScheduledPrayer<'_>,
    next: Option<ScheduledPrayer<'_>>,
    now: DateTime<Utc>,
) -> f32 {
    let start = prayer_position(prayer.index);
    let Some(next) = next else { return start };
    let end = if next.day.date == prayer.day.date {
        prayer_position(next.index).max(start)
    } else if PRAYERS[prayer.index] == Prayer::Isha && PRAYERS[next.index] == Prayer::Fajr {
        TRACK_END
    } else {
        start
    };

    let elapsed = (now - prayer.time).as_seconds_f64();
    let duration = (next.time - prayer.time).as_seconds_f64();
    start + (end - start) * (elapsed / duration) as f32
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
    use super::{
        Schedule, TRACK_END, format_countdown, prayer_position, progress_position,
        reached_checkpoints,
    };
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
    fn marker_moves_between_prayers_and_restarts_at_the_next_fajr() {
        let noon = Utc.with_ymd_and_hms(2026, 11, 23, 6, 0, 0).unwrap();
        let city = city(DHAKA_ID);
        let schedule = Schedule::new(city, noon);
        let times = schedule.today().unwrap().times.map(Option::unwrap);
        let position_at = |schedule: &Schedule, now| {
            progress_position(schedule.previous(now).unwrap(), schedule.next(now), now)
        };
        for (index, pair) in times.windows(2).enumerate() {
            let [time, next] = [pair[0], pair[1]];
            let halfway = time + (next - time) / 2;
            let start = prayer_position(index);
            assert_eq!(position_at(&schedule, time), start);
            assert_eq!(
                position_at(&schedule, halfway),
                start + (prayer_position(index + 1) - start) / 2.0
            );
        }

        let isha = times[4];
        let next_fajr = schedule.active_waqt(isha).unwrap().end;
        let halfway = isha + (next_fajr - isha) / 2;
        let start = prayer_position(4);
        assert_eq!(position_at(&schedule, isha), start);
        assert_eq!(
            position_at(&schedule, halfway),
            start + (TRACK_END - start) / 2.0
        );

        let before_fajr = next_fajr - TimeDelta::seconds(1);
        let schedule = Schedule::new(city, before_fajr);
        let isha_waqt = schedule.active_waqt(before_fajr).unwrap();
        let fajr_waqt = schedule.active_waqt(next_fajr).unwrap();
        let before_end = position_at(&schedule, before_fajr);
        assert!(before_end > TRACK_END - 0.01 && before_end <= TRACK_END);
        assert_eq!(position_at(&schedule, next_fajr), prayer_position(0));

        assert_eq!(
            reached_checkpoints(Some(isha_waqt.prayer.day), before_fajr),
            [true; 5]
        );
        assert_eq!(
            reached_checkpoints(Some(fajr_waqt.prayer.day), next_fajr),
            [true, false, false, false, false]
        );
    }

    #[test]
    fn countdown_displays_hours_minutes_and_the_final_seconds() {
        assert_eq!(format_countdown(TimeDelta::minutes(62)), "1h 02m");
        assert_eq!(format_countdown(TimeDelta::seconds(61)), "2m");
        assert_eq!(format_countdown(TimeDelta::milliseconds(100)), "1s");
    }
}
