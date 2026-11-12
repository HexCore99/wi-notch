mod platform;
mod ui;

use gpui::{prelude::*, *};
use ui::notch::Notch;

fn main() {
    Application::new().run(|cx: &mut App| {
        let width = 150.0;
        let height = 35.0;

        let display = cx.primary_display().expect("Failed to get primary display");
        let screen = display.bounds();

        let x = screen.origin.x + (screen.size.width - px(width)) / 2.0;
        let y = screen.origin.y + px(10.0);

        let bounds = Bounds::new(point(x, y), size(px(width), px(height)));

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
                platform::configure_window(window).expect("Failed to remove the window border");
                cx.new(|_| Notch::new(width, height))
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
