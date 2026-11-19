use ::windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use ::windows::Win32::Graphics::Gdi::ClientToScreen;
use ::windows::Win32::UI::HiDpi::GetDpiForWindow;
use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetWindowRect, HWND_TOPMOST, IsIconic, IsWindowVisible, SW_RESTORE,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
    SetForegroundWindow, SetWindowPos, ShowWindow, ShowWindowAsync, USER_DEFAULT_SCREEN_DPI,
};
use ::windows::Win32::{
    Foundation::{COLORREF, E_FAIL, HWND, RECT},
    Graphics::Dwm::{
        DWMNCRP_DISABLED, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_NCRENDERING_POLICY,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DwmSetWindowAttribute,
    },
    Storage::EnhancedStorage::PKEY_AppUserModel_ID,
    System::Com::StructuredStorage::{PropVariantClear, PropVariantToString},
    UI::{
        Shell::{
            DefSubclassProc,
            PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow},
            RemoveWindowSubclass, SetWindowSubclass,
        },
        WindowsAndMessaging::{HTCLIENT, WM_NCDESTROY, WM_NCHITTEST},
    },
};
use ::windows::core::{BOOL, Error};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use super::display::Monitor;
use crate::notch_geometry::notch_origin_for_height;

unsafe extern "system" fn notch_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    laparam: LPARAM,
    subclass_id: usize,
    _data: usize,
) -> LRESULT {
    if message == WM_NCHITTEST {
        return LRESULT(HTCLIENT as isize);
    } else if message == WM_NCDESTROY {
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(notch_window_proc), subclass_id);
        }
    }

    /*
     * DefSubclassProc -> Default Subclass Procedure
     * pass unhandled messages to the next handler. e.g(gpui)
     */
    unsafe { DefSubclassProc(hwnd, message, wparam, laparam) }
}

fn window_hwnd(window: &gpui::Window) -> Result<HWND, Box<dyn std::error::Error>> {
    let handle = HasWindowHandle::window_handle(window)?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("Expected a Windows window".into());
    };
    Ok(HWND(handle.hwnd.get() as *mut std::ffi::c_void))
}

pub(super) fn focus_app_window(app_id: &str) -> ::windows::core::Result<bool> {
    let mut search = (app_id, None::<HWND>);
    let _ = unsafe {
        EnumWindows(
            Some(find_matching_window),
            LPARAM(std::ptr::from_mut(&mut search) as isize),
        )
    };
    let Some(hwnd) = search.1 else {
        return Ok(false);
    };

    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindowAsync(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd).as_bool() {
            return Ok(true);
        }
    }
    Err(Error::new(
        E_FAIL,
        "Windows could not focus the existing app window",
    ))
}

unsafe extern "system" fn find_matching_window(hwnd: HWND, data: LPARAM) -> BOOL {
    // EnumWindows calls this synchronously, while the search is on the stack.
    let (app_id, matched) = unsafe { &mut *(data.0 as *mut (&str, Option<HWND>)) };
    if unsafe { IsWindowVisible(hwnd) }.as_bool()
        && window_app_id(hwnd).is_some_and(|id| id.eq_ignore_ascii_case(app_id))
    {
        *matched = Some(hwnd);
        return false.into();
    }
    true.into()
}

fn window_app_id(hwnd: HWND) -> Option<String> {
    let store: IPropertyStore = unsafe { SHGetPropertyStoreForWindow(hwnd) }.ok()?;
    let mut value = unsafe { store.GetValue(&PKEY_AppUserModel_ID) }.ok()?;
    let mut id = [0u16; 129];
    let result = unsafe { PropVariantToString(&value, &mut id) };
    let _ = unsafe { PropVariantClear(&mut value) };
    result.ok()?;
    let end = id.iter().position(|&unit| unit == 0)?;
    Some(String::from_utf16_lossy(&id[..end]))
}

pub fn place_notch(
    window: &gpui::Window,
    cx: &gpui::App,
    width: f32,
    height: f32,
    top_gap: u32,
    monitor: &Monitor,
) -> Result<(), Box<dyn std::error::Error>> {
    let hwnd = window_hwnd(window)?;
    if unsafe { IsIconic(hwnd) }.as_bool() {
        cx.foreground_executor()
            .spawn(async move {
                let _ = unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
            })
            .detach();
        return Ok(());
    }

    let mut outer = RECT::default();
    let mut inner = RECT::default();
    let mut client_origin = POINT::default();

    unsafe {
        GetWindowRect(hwnd, &mut outer)?;
        GetClientRect(hwnd, &mut inner)?;
        ClientToScreen(hwnd, &mut client_origin).ok()?;
    }

    let outer_width = outer.right - outer.left;
    let outer_height = outer.bottom - outer.top;
    let inner_width = inner.right - inner.left;
    let inner_height = inner.bottom - inner.top;

    let scale = monitor.scale;
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let window_scale = if dpi == 0 {
        window.scale_factor()
    } else {
        dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32
    };
    let scale_ratio = scale / window_scale;
    let frame_width = ((outer_width - inner_width) as f32 * scale_ratio).round() as i32;
    let frame_height = ((outer_height - inner_height) as f32 * scale_ratio).round() as i32;

    let new_width = (width * scale).round() as i32 + frame_width;
    let new_height = (height * scale).round() as i32 + frame_height;

    let origin =
        notch_origin_for_height(monitor.bounds, gpui::px(width), gpui::px(height), top_gap);
    // SetWindowPos moves the outer window. Offset it to place the visible client area.
    let inset_x = ((client_origin.x - outer.left) as f32 * scale_ratio).round() as i32;
    let inset_y = ((client_origin.y - outer.top) as f32 * scale_ratio).round() as i32;
    let x = (f32::from(origin.x) * scale).round() as i32 - inset_x;
    let y = (f32::from(origin.y) * scale).round() as i32 - inset_y;

    // Windows can move or hide the HWND even when the saved position hasn't changed.
    if outer.left == x
        && outer.top == y
        && outer_width == new_width
        && outer_height == new_height
        && unsafe { IsWindowVisible(hwnd) }.as_bool()
    {
        return Ok(());
    }

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
                    SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
                )
            };
            if let Err(error) = result {
                eprintln!("Failed to position the notch: {error}");
            }
        })
        .detach();
    Ok(())
}

pub fn remove_window_border(window: &gpui::Window) -> Result<(), Box<dyn std::error::Error>> {
    let hwnd = window_hwnd(window)?;
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
    }
    Ok(())
}

pub fn configure_window(window: &gpui::Window) -> Result<(), Box<dyn std::error::Error>> {
    remove_window_border(window)?;
    let hwnd = window_hwnd(window)?;

    unsafe {
        if !SetWindowSubclass(hwnd, Some(notch_window_proc), 1, 0).as_bool() {
            return Err("Failed to install notch window handler".into());
        }

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
