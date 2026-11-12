use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Graphics::Gdi::OUT_RASTER_PRECIS;
use windows::Win32::Security::Cryptography::KP_CLIENT_RANDOM;
use windows::Win32::UI::Shell::APPLICATION_VIEW_MIN_WIDTH;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect,  // get the content area
    GetWindowRect,  // get the entire window area
    HWND_TOPMOST,   // place the window above all non-topmost windows
    SWP_NOACTIVATE, // do not activate the window
    SWP_NOMOVE,     // do not change the window's position
    SWP_NOSIZE,     // do not change the window's size
    SWP_NOZORDER,   // do not change the window's z-order
    SetWindowPos,
};
use windows::Win32::{
    Foundation::{COLORREF, HWND, RECT},
    Graphics::Dwm::{
        DWMNCRP_DISABLED, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_NCRENDERING_POLICY,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DwmSetWindowAttribute,
    },
};

pub fn resize_notch(
    window: &gpui::Window,
    cx: &gpui::App,
    width: f32,
    height: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let handle = HasWindowHandle::window_handle(window)?;

    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("Expected a Windows window".into());
    };
    let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);

    let mut outer = RECT::default();
    let mut inner = RECT::default();

    unsafe {
        GetWindowRect(hwnd, &mut outer)?;
        GetClientRect(hwnd, &mut inner)?;
    }

    let outer_width = outer.right - outer.left;
    let outer_height = outer.bottom - outer.top;
    let inner_width = inner.right - inner.left;
    let inner_height = inner.bottom - inner.top;

    let frame_width = outer_width - inner_width;
    let frame_height = outer_height - inner_height;

    let scale = window.scale_factor(); // Get the scale factor of the window

    let new_width = (width * scale).round() as i32 + frame_width;
    let new_height = (height * scale).round() as i32 + frame_height;

    let x = outer.left + (outer_width - new_width) / 2;
    let y = outer.top;

    // Run after current gpui update has finished
    cx.foreground_executor()
        .spawn(async move {
            let result = unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    x,
                    y,
                    new_width,
                    new_height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )
            };
            if let Err(error) = result {
                eprintln!("Failed to resize the notch: {error}");
            }
        })
        .detach();
    Ok(())
}

// remove border + make window topmost
pub fn configure_window(window: &gpui::Window) -> Result<(), Box<dyn std::error::Error>> {
    let handle = HasWindowHandle::window_handle(window)?;

    /*
     * RawWindowHandle is an enum, represent Window,mac,linux handles.
     * handle.as_raw() returns a reference to the underlying handle.
     * Win32(handle) checks if the handle is a Windows handle extracts it.
     */
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("Expected a Windows window".into());
    };

    let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);
    let color = COLORREF(DWMWA_COLOR_NONE); // Set to DWMWA_COLOR_NONE to remove the border color

    let policy = DWMNCRP_DISABLED; // Disable non-client area rendering
    let corners = DWMWCP_DONOTROUND; // Disable automatic corner rounding

    unsafe {
        // Disable native frame rendering.
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY, // Set the non-client rendering policy
            std::ptr::from_ref(&policy).cast(), // Cast the reference to a pointer
            std::mem::size_of_val(&policy) as u32, // Specify the size of the policy value
        )?;

        // Suppress the native border color.
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR, // Set the border color attribute
            std::ptr::from_ref(&color).cast(),
            std::mem::size_of_val(&color) as u32,
        )?;

        // Disable Windows' automatic corner rounding.
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE, // Set the window corner preference attribute
            std::ptr::from_ref(&corners).cast(),
            std::mem::size_of_val(&corners) as u32,
        )?;

        // Set the window to be topmost without activating it.
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )?;
    }

    Ok(())
}
