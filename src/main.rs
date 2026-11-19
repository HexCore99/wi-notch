mod appearance;
mod notch_geometry;
mod ui;
mod windows;

use appearance::AppearanceSettings;
use gpui::{prelude::*, *};
use notch_geometry::notch_origin;
use ui::notch::Notch;

fn main() {
    Application::new().run(|cx: &mut App| {
        let width = 150.0;
        let height = 35.0;

        let display = cx.primary_display().expect("Failed to get primary display");
        let settings = AppearanceSettings::load();
        let origin = notch_origin(display.bounds(), px(width), settings.top_gap);
        let bounds = Bounds::new(origin, size(px(width), px(height)));
        let appearance = cx.new(|_| settings);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
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
                cx.new(|cx| Notch::new(width, height, appearance, cx))
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
