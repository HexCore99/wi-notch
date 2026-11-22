use std::sync::OnceLock;

use adhan::{Madhab, Method, Parameters, Prayer, prayer_times};
use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc};
use chrono_tz::Tz;

pub const DHAKA_ID: u32 = 1_185_241;

#[derive(Debug)]
pub struct City {
    pub id: u32,
    pub name: &'static str,
    pub region: &'static str,
    pub country: &'static str,
    pub zone: Tz,
    latitude: f64,
    longitude: f64,
    search_name: String,
    search_ascii: String,
    search_location: String,
}

impl City {
    fn from_row(row: &'static str) -> Option<Self> {
        let [
            id,
            name,
            ascii_name,
            region,
            country,
            zone,
            latitude,
            longitude,
        ] = row.split('\t').collect::<Vec<_>>().try_into().ok()?;
        Some(Self {
            id: id.parse().ok()?,
            name,
            region,
            country,
            zone: zone.parse().ok()?,
            latitude: latitude.parse().ok()?,
            longitude: longitude.parse().ok()?,
            search_name: name.to_lowercase(),
            search_ascii: ascii_name.to_lowercase(),
            search_location: format!("{region} {country}").to_lowercase(),
        })
    }
}

fn cities() -> &'static [City] {
    static CITIES: OnceLock<Vec<City>> = OnceLock::new();
    CITIES.get_or_init(|| {
        include_str!("../assets/prayer-cities.tsv")
            .lines()
            .filter(|row| !row.starts_with('#'))
            .filter_map(City::from_row)
            .collect()
    })
}

pub fn city(id: u32) -> &'static City {
    let cities = cities();
    cities
        .iter()
        .find(|city| city.id == id)
        .or_else(|| cities.iter().find(|city| city.id == DHAKA_ID))
        .expect("the bundled city list includes Dhaka")
}

pub fn search_cities(query: &str, limit: usize) -> Vec<&'static City> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return cities().iter().take(limit).collect();
    }
    let terms: Vec<_> = query
        .split(|ch: char| ch.is_whitespace() || ch == ',')
        .filter(|term| !term.is_empty())
        .collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let mut exact = Vec::new();
    let mut prefixes = Vec::new();
    let mut other = Vec::new();

    // The bundled rows are already ordered by population within each match class.
    for city in cities() {
        let results = if city.search_name == query || city.search_ascii == query {
            &mut exact
        } else if city.search_name.starts_with(&query) || city.search_ascii.starts_with(&query) {
            &mut prefixes
        } else if terms.iter().all(|term| {
            city.search_name.contains(term)
                || city.search_ascii.contains(term)
                || city.search_location.contains(term)
        }) {
            &mut other
        } else {
            continue;
        };
        if results.len() < limit {
            results.push(city);
        }
    }
    exact
        .into_iter()
        .chain(prefixes)
        .chain(other)
        .take(limit)
        .collect()
}

pub const PRAYERS: [Prayer; 5] = [
    Prayer::Fajr,
    Prayer::Dhuhr,
    Prayer::Asr,
    Prayer::Maghrib,
    Prayer::Isha,
];

pub struct DaySchedule {
    pub date: NaiveDate,
    pub times: [Option<DateTime<Utc>>; PRAYERS.len()],
    sunrise: Option<DateTime<Utc>>,
    sunset: Option<DateTime<Utc>>,
}

impl DaySchedule {
    fn calculate(city: &City, date: NaiveDate) -> Option<Self> {
        let prayers = prayer_times(
            date.year(),
            date.month(),
            date.day(),
            city.latitude,
            city.longitude,
            &Parameters::for_method(Method::Karachi).with_madhab(Madhab::Hanafi),
        );

        // Adhan wraps its UTC clock readings at midnight. Put Dhuhr back on
        // the day nearest solar noon, then place the other prayers around it.
        let midnight = date.and_hms_opt(0, 0, 0)?.and_utc();
        let noon_seconds = (43_200.0 - city.longitude * 240.0).round() as i64;
        let dhuhr_seconds = seconds(prayers.dhuhr?);
        let dhuhr_offset = (dhuhr_seconds - noon_seconds + 43_200).rem_euclid(86_400) - 43_200;
        let dhuhr = midnight.checked_add_signed(TimeDelta::seconds(noon_seconds + dhuhr_offset))?;
        let to_utc = |reading, morning| {
            let time = seconds(reading);
            let offset = if morning {
                -(dhuhr_seconds - time).rem_euclid(86_400)
            } else {
                (time - dhuhr_seconds).rem_euclid(86_400)
            };
            dhuhr.checked_add_signed(TimeDelta::seconds(offset))
        };
        Some(Self {
            date: dhuhr.with_timezone(&city.zone).date_naive(),
            times: PRAYERS.map(|prayer| to_utc(prayers.time_for(prayer)?, prayer == Prayer::Fajr)),
            sunrise: prayers.sunrise.and_then(|time| to_utc(time, true)),
            sunset: prayers.sunset.and_then(|time| to_utc(time, false)),
        })
    }

    fn start(&self, prayer: Prayer) -> Option<DateTime<Utc>> {
        let index = PRAYERS.iter().position(|&entry| entry == prayer)?;
        self.times[index]
    }
}

fn seconds((hours, minutes, seconds): (i64, u32, u32)) -> i64 {
    hours * 3600 + i64::from(minutes) * 60 + i64::from(seconds)
}

pub struct Schedule {
    pub city: &'static City,
    date: NaiveDate,
    days: Vec<DaySchedule>,
}

#[derive(Clone, Copy)]
pub struct ScheduledPrayer<'a> {
    pub day: &'a DaySchedule,
    pub index: usize,
    pub time: DateTime<Utc>,
}

pub struct Waqt<'a> {
    pub prayer: ScheduledPrayer<'a>,
    pub end: DateTime<Utc>,
}

impl Schedule {
    pub fn new(city: &'static City, now: DateTime<Utc>) -> Self {
        let date = now.with_timezone(&city.zone).date_naive();
        // At the date line, a solar day's starting date can be a day away
        // from the civil date. Keep yesterday's schedule for Isha after midnight.
        let days = (-2..=2)
            .filter_map(|offset| date.checked_add_signed(TimeDelta::days(offset)))
            .filter_map(|date| DaySchedule::calculate(city, date))
            .filter(|day| (day.date - date).num_days().abs() <= 1)
            .collect();
        Self { city, date, days }
    }

    pub fn is_current(&self, city_id: u32, now: DateTime<Utc>) -> bool {
        self.city.id == city_id && self.date == now.with_timezone(&self.city.zone).date_naive()
    }

    pub fn today(&self) -> Option<&DaySchedule> {
        self.day(self.date)
    }

    fn day(&self, date: NaiveDate) -> Option<&DaySchedule> {
        self.days.iter().find(|day| day.date == date)
    }

    fn prayers(&self) -> impl Iterator<Item = ScheduledPrayer<'_>> {
        self.days.iter().flat_map(|day| {
            day.times
                .iter()
                .enumerate()
                .filter_map(move |(index, time)| {
                    time.map(|time| ScheduledPrayer { day, index, time })
                })
        })
    }

    pub fn previous(&self, now: DateTime<Utc>) -> Option<ScheduledPrayer<'_>> {
        self.prayers()
            .filter(|prayer| prayer.time <= now)
            .max_by_key(|prayer| prayer.time)
    }

    pub fn next(&self, now: DateTime<Utc>) -> Option<ScheduledPrayer<'_>> {
        self.prayers()
            .filter(|prayer| prayer.time > now)
            .min_by_key(|prayer| prayer.time)
    }

    pub fn active_waqt(&self, now: DateTime<Utc>) -> Option<Waqt<'_>> {
        let prayer = self.previous(now)?;
        let day = prayer.day;
        let end = match PRAYERS[prayer.index] {
            Prayer::Fajr => day.sunrise,
            Prayer::Dhuhr => day.start(Prayer::Asr),
            Prayer::Asr => day.sunset,
            Prayer::Maghrib => day.start(Prayer::Isha),
            Prayer::Isha => self.day(day.date.succ_opt()?)?.start(Prayer::Fajr),
            Prayer::Sunrise => None,
        }?;
        (now < end).then_some(Waqt { prayer, end })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Offset, TimeZone};

    fn utc(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, 0, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn city_picker_finds_worldwide_results_and_defaults_to_dhaka() {
        assert_eq!(city(0).name, "Dhaka");
        assert_eq!(search_cities("  NEW york ", 1)[0].id, 5_128_581);
        assert_eq!(search_cities("Tromso", 1)[0].zone, chrono_tz::Europe::Oslo);
        assert!(search_cities("qxqxq", 12).is_empty());
    }

    #[test]
    fn transitions_at_prayer_time_and_uses_tomorrow_after_isha() {
        let city = city(DHAKA_ID);
        let now = utc(2026, 11, 23, 0);
        let schedule = Schedule::new(city, now);
        let today = schedule.today().unwrap();
        let times = today.times.map(Option::unwrap);
        assert!(times.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            times[0].with_timezone(&city.zone).format("%H").to_string(),
            "04"
        );
        assert_eq!(
            schedule
                .next(times[2] - TimeDelta::seconds(1))
                .unwrap()
                .index,
            2
        );
        assert_eq!(schedule.next(times[2]).unwrap().index, 3);

        let next = schedule.next(times[4]).unwrap();
        assert_eq!(next.index, 0);
        assert_eq!(next.day.date, today.date.succ_opt().unwrap());
        let midnight = utc(2026, 11, 23, 18);
        assert!(!schedule.is_current(city.id, midnight));
        assert_eq!(
            Schedule::new(city, midnight).next(midnight).unwrap().time,
            next.time
        );
    }

    #[test]
    fn daylight_saving_and_new_utc_day_are_applied_to_new_york() {
        let city = city(5_128_581);
        let before = utc(2026, 3, 7, 12);
        let after = utc(2026, 3, 8, 12);
        for (now, offset) in [(before, -5), (after, -4)] {
            let day = Schedule::new(city, now);
            let times = day.today().unwrap().times.map(Option::unwrap);
            assert_eq!(
                times[0]
                    .with_timezone(&city.zone)
                    .offset()
                    .fix()
                    .local_minus_utc(),
                offset * 3600
            );
            assert_eq!(times[4].with_timezone(&city.zone).date_naive(), day.date);
            assert!(times[4] > times[0]);
        }
        assert_eq!(
            Schedule::new(city, after).today().unwrap().times[4]
                .unwrap()
                .date_naive(),
            after.date_naive().succ_opt().unwrap()
        );
    }

    #[test]
    fn a_late_isha_is_still_upcoming_after_local_midnight() {
        let city = city(DHAKA_ID);
        let now = utc(2026, 11, 23, 18);
        let mut schedule = Schedule::new(city, now);
        let late_isha = now + TimeDelta::minutes(15);
        let previous_date = schedule.date.pred_opt().unwrap();
        let previous = schedule
            .days
            .iter_mut()
            .find(|day| day.date == previous_date)
            .unwrap();
        previous.times[4] = Some(late_isha);
        assert_eq!(schedule.next(now).unwrap().time, late_isha);
        assert_eq!(schedule.next(late_isha).unwrap().index, 0);
    }

    #[test]
    fn waqt_starts_at_prayer_time_and_fajr_ends_at_sunrise() {
        let schedule = Schedule::new(city(DHAKA_ID), utc(2026, 11, 23, 0));
        let day = schedule.today().unwrap();
        let [fajr, dhuhr, asr, maghrib, isha] = day.times.map(Option::unwrap);
        let sunrise = day.sunrise.unwrap();
        for (start, end, expected) in [
            (fajr, sunrise, Prayer::Fajr),
            (dhuhr, asr, Prayer::Dhuhr),
            (asr, day.sunset.unwrap(), Prayer::Asr),
            (maghrib, isha, Prayer::Maghrib),
        ] {
            let waqt = schedule.active_waqt(start).unwrap();
            assert_eq!(PRAYERS[waqt.prayer.index], expected);
            assert_eq!(waqt.end, end);
        }
        assert!(
            schedule
                .active_waqt(sunrise - TimeDelta::seconds(1))
                .is_some()
        );
        assert!(schedule.active_waqt(sunrise).is_none());
        assert!(
            schedule
                .active_waqt(dhuhr - TimeDelta::seconds(1))
                .is_none()
        );
        assert_eq!(schedule.next(sunrise).unwrap().time, dhuhr);
    }

    #[test]
    fn isha_stays_active_across_midnight_until_the_next_fajr() {
        let city = city(DHAKA_ID);
        let midnight = utc(2026, 11, 23, 18);
        let schedule = Schedule::new(city, midnight);
        let waqt = schedule.active_waqt(midnight).unwrap();
        let fajr = schedule.today().unwrap().start(Prayer::Fajr).unwrap();
        assert_eq!(PRAYERS[waqt.prayer.index], Prayer::Isha);
        assert_eq!(waqt.prayer.day.date.succ_opt().unwrap(), schedule.date);
        assert_eq!(waqt.end, fajr);
        assert_eq!(
            PRAYERS[schedule.active_waqt(fajr).unwrap().prayer.index],
            Prayer::Fajr
        );
    }

    #[test]
    fn civil_date_is_correct_near_the_date_line() {
        let city = city(4_035_413); // Apia, Samoa
        assert_eq!(city.name, "Apia");
        let now = utc(2026, 12, 31, 12);
        let schedule = Schedule::new(city, now);
        let day = schedule.today().unwrap();
        assert_eq!(day.date, NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
        let fajr = day.times[0].unwrap();
        assert_eq!(fajr.date_naive(), now.date_naive());
        assert_eq!(fajr.with_timezone(&city.zone).date_naive(), day.date);
        assert_eq!(schedule.next(now).unwrap().time, fajr);
    }

    #[test]
    fn missing_polar_prayers_leave_available_times_intact() {
        let city = search_cities("Tromso", 1)[0];
        let now = utc(2026, 6, 21, 7);
        let schedule = Schedule::new(city, now);
        let day = schedule.today().unwrap();
        assert!(day.times[0].is_none());
        assert!(day.times[3].is_none());
        assert_eq!(schedule.next(now).unwrap().index, 1);
    }
}
