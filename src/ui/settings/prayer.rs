use gpui::{prelude::*, *};

use super::text_input::{TextChanged, TextInput};
use super::{
    CONTROL_BACKGROUND, CONTROL_HOVER, MUTED_TEXT, save_error, setting_group, setting_row,
    update_settings,
};
use crate::appearance::AppearanceSettings;
use crate::prayer::{City, city, search_cities};

pub(super) struct PrayerPage {
    settings: Entity<AppearanceSettings>,
    search: Entity<TextInput>,
    results: Vec<&'static City>,
    results_scroll: ScrollHandle,
    save_failed: bool,
}

impl PrayerPage {
    pub(super) fn new(settings: Entity<AppearanceSettings>, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new("search-prayer-cities", "Search for a city…", cx));
        cx.subscribe(&search, |page, search, _: &TextChanged, cx| {
            page.results = search_cities(search.read(cx).value(), 30);
            page.results_scroll.set_offset(point(px(0.0), px(0.0)));
            cx.notify();
        })
        .detach();
        Self {
            settings,
            search,
            results: Vec::new(),
            results_scroll: ScrollHandle::new(),
            save_failed: false,
        }
    }

    fn select(&mut self, city: &City, cx: &mut Context<Self>) {
        self.save_failed = update_settings(
            &self.settings,
            |settings| {
                settings.prayer_city_id = city.id;
            },
            cx,
        )
        .is_err();
        cx.notify();
    }
}

impl Render for PrayerPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = city(self.settings.read(cx).prayer_city_id);
        let city_label = format!("{}, {}", selected.name, selected.country);
        let empty_query = self.search.read(cx).value().trim().is_empty();
        let popular;
        let results = if empty_query {
            popular = search_cities("", 12);
            &popular
        } else {
            &self.results
        };
        let first = if empty_query {
            None
        } else {
            results.first().copied()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .on_key_down(cx.listener(move |page, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter"
                    && let Some(city) = first
                {
                    page.select(city, cx);
                    cx.stop_propagation();
                }
            }))
            .child(setting_group(
                "Prayer schedule",
                [
                    setting_row(
                        "City",
                        div()
                            .max_w(px(275.0))
                            .truncate()
                            .text_color(rgb(0xba9cff))
                            .child(city_label),
                    ),
                    setting_row(
                        "Calculation",
                        div()
                            .text_color(rgb(MUTED_TEXT))
                            .child("Karachi · Hanafi Asr"),
                    ),
                ],
            ))
            .child(self.search.clone())
            .child(
                div()
                    .id("prayer-city-results")
                    .h(px(288.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.results_scroll)
                    .rounded_xl()
                    .bg(rgb(0x28282a))
                    .when(results.is_empty(), |list| {
                        list.child(
                            div()
                                .p(px(12.0))
                                .text_color(rgb(MUTED_TEXT))
                                .child("No cities found"),
                        )
                    })
                    .children(results.iter().copied().map(|city| {
                        let location = match city.region {
                            "" => city.country.to_owned(),
                            region => format!("{region}, {}", city.country),
                        };
                        div()
                            .id(city.id as usize)
                            .px(px(12.0))
                            .py(px(5.0))
                            .min_h(px(42.0))
                            .cursor_pointer()
                            .rounded_md()
                            .when(city.id == selected.id, |row| {
                                row.bg(rgb(CONTROL_BACKGROUND))
                            })
                            .hover(|style| style.bg(rgb(CONTROL_HOVER)))
                            .child(div().truncate().child(city.name))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.0))
                                    .text_color(rgb(MUTED_TEXT))
                                    .child(location),
                            )
                            .on_click(cx.listener(move |page, _, _, cx| page.select(city, cx)))
                    })),
            )
            .when(self.save_failed, |page| page.child(save_error()))
    }
}
