use chrono::{Datelike, Days, Months, NaiveDate};
use gpui::{prelude::*, *};

use super::notch::{Notch, PAGE_BODY_WIDTH};

pub(super) const PANEL_HEIGHT: f32 = 224.0;
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

#[derive(Clone, Copy, Default)]
pub(super) struct Calendar {
    // Follow today until the user browses to another month.
    month: Option<NaiveDate>,
}

impl Calendar {
    fn month(self, today: NaiveDate) -> NaiveDate {
        self.month.unwrap_or_else(|| first_of_month(today))
    }

    pub fn show_month(&mut self, month: NaiveDate, today: NaiveDate) {
        self.month = (month != first_of_month(today)).then_some(month);
    }

    pub(super) fn panel(self, today: NaiveDate, cx: &Context<Notch>) -> Div {
        let month = self.month(today);
        let (previous, next) = adjacent_months(month);
        let days = month_grid(month);

        div()
            .w(px(PAGE_BODY_WIDTH))
            .h(px(PANEL_HEIGHT))
            .flex_none()
            .flex()
            .flex_col()
            .py(px(6.0))
            .child(
                div()
                    .h(px(34.0))
                    .flex()
                    .justify_between()
                    .items_center()
                    .text_size(px(15.0))
                    .child(month_arrow("previous-month", "‹", previous, today, cx))
                    .child(month.format("%B %Y").to_string())
                    .child(month_arrow("next-month", "›", next, today, cx)),
            )
            .child(
                div()
                    .flex()
                    .h(px(22.0))
                    .text_size(px(11.0))
                    .text_color(rgb(0xa7a2ae))
                    .children(WEEKDAYS.map(|day| cell().child(day))),
            )
            .children(days.chunks_exact(7).map(|week| {
                div().flex().h(px(26.0)).children(week.iter().map(|&day| {
                    cell().when_some(day, |cell, date| {
                        cell.child(
                            div()
                                .size(px(24.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .when(date == today, |day| day.bg(rgb(0x7941b1)))
                                .child(date.day().to_string()),
                        )
                    })
                }))
            }))
    }
}

fn cell() -> Div {
    div()
        .flex_1()
        .min_w(px(0.0))
        .flex()
        .items_center()
        .justify_center()
}

fn month_arrow(
    id: &'static str,
    icon: &'static str,
    month: Option<NaiveDate>,
    today: NaiveDate,
    cx: &Context<Notch>,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(26.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .text_size(px(20.0))
        .child(icon)
        .when(month.is_none(), |arrow| arrow.opacity(0.4))
        .when_some(month, |arrow, month| {
            arrow
                .cursor_pointer()
                .hover(|style| style.bg(rgba(0xffffff20)))
                .on_click(cx.listener(move |notch, _, _, cx| {
                    notch.show_calendar_month(month, today, cx);
                }))
        })
}

fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).expect("every month has a first day")
}

fn adjacent_months(month: NaiveDate) -> (Option<NaiveDate>, Option<NaiveDate>) {
    let one_month = Months::new(1);
    (
        month.checked_sub_months(one_month),
        month.checked_add_months(one_month),
    )
}

fn month_grid(month: NaiveDate) -> [Option<NaiveDate>; 42] {
    let start = month.weekday().num_days_from_sunday() as usize;
    std::array::from_fn(|slot: usize| {
        let offset = slot.checked_sub(start)?;
        month
            .checked_add_days(Days::new(offset as u64))
            .filter(|day| day.month() == month.month())
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{Calendar, adjacent_months, month_grid};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn sunday_first_grid_handles_leap_february_and_six_row_months() {
        let leap_february = month_grid(date(2024, 2, 1));
        assert_eq!(leap_february[..4], [None; 4]);
        assert_eq!(leap_february[4], Some(date(2024, 2, 1)));
        assert_eq!(leap_february[32], Some(date(2024, 2, 29)));
        assert!(leap_february[33..].iter().all(Option::is_none));

        let may = month_grid(date(2026, 5, 1));
        assert_eq!(may[35], Some(date(2026, 5, 31)));
        assert_eq!(may[36], None);
    }

    #[test]
    fn navigation_crosses_years_and_current_month_follows_today() {
        let december = date(2026, 12, 1);
        let january = date(2027, 1, 1);
        assert_eq!(adjacent_months(december).1, Some(january));
        assert_eq!(adjacent_months(january).0, Some(december));

        let mut calendar = Calendar::default();
        assert_eq!(calendar.month(date(2026, 12, 31)), december);
        assert_eq!(calendar.month(january), january);
        calendar.show_month(december, january);
        assert_eq!(calendar.month(date(2027, 2, 1)), december);
        calendar.show_month(january, january);
        assert_eq!(calendar.month(date(2027, 2, 1)), date(2027, 2, 1));
    }
}
