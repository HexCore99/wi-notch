use std::collections::HashMap;

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use super::{PRAYER_NAMES, Schedule};

const LEAD_TIME: TimeDelta = TimeDelta::minutes(10);
const LATE_WINDOW: TimeDelta = TimeDelta::minutes(1);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct ReminderSettings {
    pub before: bool,
    pub at_start: bool,
}

impl ReminderSettings {
    pub fn enabled(self) -> bool {
        self.before || self.at_start
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum ReminderKind {
    Before,
    Start,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrayerReminder {
    index: usize,
    kind: ReminderKind,
}

impl PrayerReminder {
    pub fn message(self) -> String {
        let name = PRAYER_NAMES[self.index];
        match self.kind {
            ReminderKind::Before => format!("{name} starts in 10 minutes"),
            ReminderKind::Start => format!("Time for {name}"),
        }
    }
}

pub struct PrayerReminders {
    city_id: u32,
    settings: ReminderSettings,
    checked_at: DateTime<Utc>,
    // Keep the latest handled time per city, prayer and kind across clock changes.
    handled: HashMap<(u32, usize, ReminderKind), DateTime<Utc>>,
}

impl PrayerReminders {
    pub fn new(city_id: u32, settings: ReminderSettings, now: DateTime<Utc>) -> Self {
        Self {
            city_id,
            settings,
            checked_at: now,
            handled: HashMap::new(),
        }
    }

    pub fn configure(
        &mut self,
        city_id: u32,
        settings: ReminderSettings,
        now: DateTime<Utc>,
    ) -> bool {
        if self.city_id == city_id && self.settings == settings {
            return false;
        }
        self.city_id = city_id;
        self.settings = settings;
        self.checked_at = now;
        true
    }

    pub fn check(&mut self, schedule: &Schedule, now: DateTime<Utc>) -> Option<PrayerReminder> {
        let previous = std::mem::replace(&mut self.checked_at, now);
        let mut latest = None;
        for prayer in schedule.prayers() {
            let candidates = [
                (
                    self.settings.before,
                    ReminderKind::Before,
                    prayer.time - LEAD_TIME,
                ),
                (self.settings.at_start, ReminderKind::Start, prayer.time),
            ];
            for (enabled, kind, due) in candidates {
                if !enabled || due <= previous || due > now || now - due > LATE_WINDOW {
                    continue;
                }

                let key = (schedule.city.id, prayer.index, kind);
                if self.handled.get(&key).is_some_and(|&last| due <= last) {
                    continue;
                }
                self.handled.insert(key, due);

                let order = (due, kind);
                if latest.is_none_or(|(previous, _)| order > previous) {
                    latest = Some((
                        order,
                        PrayerReminder {
                            index: prayer.index,
                            kind,
                        },
                    ));
                }
            }
        }
        latest.map(|(_, reminder)| reminder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prayer::{DHAKA_ID, DaySchedule, city};
    use chrono::TimeZone;

    const BOTH: ReminderSettings = ReminderSettings {
        before: true,
        at_start: true,
    };
    const JUST_BEFORE: ReminderSettings = ReminderSettings {
        at_start: false,
        ..BOTH
    };
    const JUST_START: ReminderSettings = ReminderSettings {
        before: false,
        ..BOTH
    };
    const SECOND: TimeDelta = TimeDelta::seconds(1);

    fn midnight() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 24, 18, 0, 0).unwrap()
    }

    fn schedule(city_id: u32, times: [Option<DateTime<Utc>>; 5]) -> Schedule {
        let city = city(city_id);
        let date = midnight().with_timezone(&city.zone).date_naive();
        Schedule {
            city,
            date,
            days: vec![DaySchedule {
                date,
                times,
                sunrise: None,
                sunset: None,
            }],
        }
    }

    #[test]
    fn switches_independently_trigger_at_the_two_due_times() {
        let starts = midnight() + TimeDelta::hours(4);
        let early = starts - LEAD_TIME;
        let schedule = schedule(DHAKA_ID, [Some(starts), None, None, None, None]);
        for settings in [ReminderSettings::default(), JUST_BEFORE, JUST_START, BOTH] {
            let mut reminders = PrayerReminders::new(DHAKA_ID, settings, early - SECOND);
            assert_eq!(reminders.check(&schedule, early - SECOND), None);
            let before = reminders
                .check(&schedule, early)
                .map(PrayerReminder::message);
            assert_eq!(
                before.as_deref(),
                settings.before.then_some("Fajr starts in 10 minutes")
            );
            assert_eq!(reminders.check(&schedule, early + SECOND), None);
            let at_start = reminders
                .check(&schedule, starts)
                .map(PrayerReminder::message);
            assert_eq!(
                at_start.as_deref(),
                settings.at_start.then_some("Time for Fajr")
            );
            assert_eq!(reminders.check(&schedule, starts + SECOND), None);
        }
    }

    #[test]
    fn changing_preferences_skips_past_events_and_preserves_one_shot_delivery() {
        let starts = midnight();
        let dhaka = schedule(DHAKA_ID, [Some(starts), None, None, None, None]);
        let new_york_id = 5_128_581;
        let new_york = schedule(new_york_id, [Some(starts), None, None, None, None]);
        let mut reminders = PrayerReminders::new(DHAKA_ID, JUST_START, starts - SECOND);
        assert!(reminders.check(&dhaka, starts).is_some());

        assert!(reminders.configure(new_york_id, JUST_START, starts));
        assert_eq!(reminders.check(&new_york, starts + SECOND), None);
        assert!(reminders.configure(DHAKA_ID, ReminderSettings::default(), starts + SECOND));
        assert!(reminders.configure(DHAKA_ID, JUST_START, starts + SECOND));
        assert_eq!(reminders.check(&dhaka, starts - SECOND), None);
        assert_eq!(reminders.check(&dhaka, starts), None);

        assert!(reminders.configure(new_york_id, JUST_START, starts - SECOND));
        assert!(!reminders.configure(new_york_id, JUST_START, starts));
        assert!(reminders.check(&new_york, starts).is_some());
    }

    #[test]
    fn delayed_checks_skip_stale_events_and_prefer_the_latest_then_start() {
        let starts = midnight();
        let later = starts + TimeDelta::seconds(30);
        let schedule = schedule(
            DHAKA_ID,
            [
                Some(starts),
                Some(starts + LEAD_TIME),
                Some(later),
                None,
                None,
            ],
        );
        for (now, expected) in [
            (starts, Some("Time for Fajr")),
            (later + LATE_WINDOW, Some("Time for Asr")),
            (later + LATE_WINDOW + SECOND, None),
        ] {
            let mut reminders = PrayerReminders::new(DHAKA_ID, BOTH, starts - SECOND);
            let reminder = reminders.check(&schedule, now).map(PrayerReminder::message);
            assert_eq!(reminder.as_deref(), expected);
        }
    }

    #[test]
    fn next_day_fajr_can_remind_before_midnight_and_start_after_it() {
        let midnight = midnight();
        let starts = midnight + TimeDelta::minutes(5);
        let early = starts - LEAD_TIME;
        let schedule = schedule(DHAKA_ID, [Some(starts), None, None, None, None]);
        let mut reminders = PrayerReminders::new(DHAKA_ID, BOTH, early - SECOND);
        for (now, expected) in [
            (early, "Fajr starts in 10 minutes"),
            (starts, "Time for Fajr"),
        ] {
            assert_eq!(reminders.check(&schedule, now).unwrap().message(), expected);
        }
    }
}
