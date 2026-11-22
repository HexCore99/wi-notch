#![windows_subsystem = "windows"]
mod appearance;
mod icons;
mod notch_geometry;
mod prayer;
mod ui;
mod windows;

use appearance::AppearanceSettings;
use gpui::{prelude::*, *};
use notch_geometry::{COLLAPSED_SIZE, notch_origin};
use ui::notch::Notch;
use windows::display::{monitors, selected_monitor};

fn main() {
    let app = Application::new().with_assets(icons::Icons);
    app.run(|cx: &mut App| {
        let width = 150.0;
        let height = 35.0;

        let mut settings = AppearanceSettings::load();
        let monitors = monitors();
        let monitor = selected_monitor(&monitors, settings.monitor.as_deref())
            .expect("Failed to get a display");
        if settings.monitor.as_deref() == Some(monitor.device.as_str())
            && monitor.identity() != monitor.device
        {
            settings.monitor = Some(monitor.identity().to_owned());
            if let Err(error) = settings.save() {
                eprintln!("Failed to save the monitor selection: {error}");
            }
        }
        // GPUI's Windows display IDs use the same monitor enumeration order.
        let display_id = cx
            .displays()
            .into_iter()
            .find(|display| u32::from(display.id()) == monitor.index)
            .map(|display| display.id());
        let (initial_width, initial_height) = if settings.always_minimal {
            COLLAPSED_SIZE
        } else {
            (width, height)
        };
        let origin = notch_origin(monitor.bounds, px(initial_width), settings.top_gap);
        let bounds = Bounds::new(origin, size(px(initial_width), px(initial_height)));
        let appearance = cx.new(|_| settings);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                display_id,
                titlebar: None,
                kind: WindowKind::PopUp,
                is_resizable: false,
                is_minimizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            |window, cx| {
                windows::window::configure_window(window)
                    .expect("Failed to remove the window border");
                cx.new(|cx| Notch::new(width, height, appearance, window, cx))
            },
        )
        .expect("Failed to open the notch window");
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
