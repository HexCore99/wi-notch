use ::windows::Win32::{
    Foundation::{LPARAM, RECT},
    Graphics::Gdi::{
        DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR,
        MONITORINFOEXW,
    },
    UI::{
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        WindowsAndMessaging::{
            EDD_GET_DEVICE_INTERFACE_NAME, MONITORINFOF_PRIMARY, USER_DEFAULT_SCREEN_DPI,
        },
    },
};
use ::windows::core::{BOOL, PCWSTR};
use gpui::{Bounds, Pixels, point, px, size};

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub index: u32,
    pub device: String,
    device_path: Option<String>,
    pub bounds: Bounds<Pixels>,
    pub scale: f32,
    pub primary: bool,
}

impl Monitor {
    pub fn identity(&self) -> &str {
        self.device_path.as_deref().unwrap_or(&self.device)
    }

    pub fn label(&self) -> String {
        let number = self
            .device
            .strip_prefix("DISPLAY")
            .and_then(|number| number.parse::<u32>().ok())
            .unwrap_or(self.index + 1);
        let primary = if self.primary { " (Primary)" } else { "" };
        format!("Display {number}{primary}")
    }
}

pub fn monitors() -> Vec<Monitor> {
    let mut handles = Vec::<HMONITOR>::new();
    // The Windows callback runs before EnumDisplayMonitors returns.
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&mut handles as *mut _ as isize),
        );
    }
    handles
        .into_iter()
        .enumerate()
        .filter_map(|(index, handle)| read_monitor(index as u32, handle))
        .collect()
}

pub fn selected_monitor<'a>(
    monitors: &'a [Monitor],
    selection: Option<&str>,
) -> Option<&'a Monitor> {
    selection
        .and_then(|selection| {
            monitors
                .iter()
                .find(|monitor| monitor.identity() == selection || monitor.device == selection)
        })
        .or_else(|| monitors.iter().find(|monitor| monitor.primary))
        .or_else(|| monitors.first())
}

unsafe extern "system" fn collect_monitor(
    handle: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let handles = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
    handles.push(handle);
    BOOL(1)
}

fn read_monitor(index: u32, handle: HMONITOR) -> Option<Monitor> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    let mut dpi_x = 0;
    let mut dpi_y = 0;
    unsafe {
        GetMonitorInfoW(handle, (&raw mut info).cast()).ok().ok()?;
        GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).ok()?;
    }
    let scale = dpi_x as f32 / USER_DEFAULT_SCREEN_DPI as f32;
    let rect = info.monitorInfo.rcMonitor;
    let device = String::from_utf16_lossy(&info.szDevice);
    Some(Monitor {
        index,
        device_path: monitor_device_path(&info.szDevice),
        device: device
            .trim_end_matches('\0')
            .trim_start_matches(r"\\.\")
            .to_owned(),
        bounds: Bounds::new(
            point(px(rect.left as f32 / scale), px(rect.top as f32 / scale)),
            size(
                px((rect.right - rect.left) as f32 / scale),
                px((rect.bottom - rect.top) as f32 / scale),
            ),
        ),
        scale,
        primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}

fn monitor_device_path(device: &[u16]) -> Option<String> {
    let mut monitor = DISPLAY_DEVICEW {
        cb: size_of::<DISPLAY_DEVICEW>() as u32,
        ..Default::default()
    };
    let found = unsafe {
        EnumDisplayDevicesW(
            PCWSTR(device.as_ptr()),
            0,
            &mut monitor,
            EDD_GET_DEVICE_INTERFACE_NAME,
        )
    };
    let length = monitor
        .DeviceID
        .iter()
        .position(|&character| character == 0)
        .unwrap_or(monitor.DeviceID.len());
    (found.as_bool() && length > 0).then(|| String::from_utf16_lossy(&monitor.DeviceID[..length]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notch_geometry::notch_origin;

    #[test]
    fn named_monitor_wins_and_missing_monitor_falls_back_to_primary() {
        let secondary = Monitor {
            index: 0,
            device: "DISPLAY2".into(),
            device_path: Some("monitor-path-b".into()),
            bounds: Bounds::new(point(px(-1280.0), px(0.0)), size(px(1280.0), px(720.0))),
            scale: 1.5,
            primary: false,
        };
        let primary = Monitor {
            index: 1,
            device: "DISPLAY1".into(),
            device_path: Some("monitor-path-a".into()),
            primary: true,
            ..secondary.clone()
        };
        let monitors = [secondary, primary];
        assert_eq!(
            selected_monitor(&monitors, Some("DISPLAY2")),
            Some(&monitors[0])
        );
        assert_eq!(
            selected_monitor(&monitors, Some("DISPLAY3")),
            Some(&monitors[1])
        );
        assert_eq!(selected_monitor(&monitors, None), Some(&monitors[1]));
    }

    #[test]
    fn physical_selection_survives_primary_and_source_changes() {
        let original = Monitor {
            index: 0,
            device: "DISPLAY1".into(),
            device_path: Some("monitor-path-a".into()),
            bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(1920.0), px(1080.0))),
            scale: 1.0,
            primary: true,
        };
        let saved = original.identity().to_owned();
        let moved = Monitor {
            index: 1,
            device: "DISPLAY2".into(),
            bounds: Bounds::new(point(px(-1920.0), px(0.0)), original.bounds.size),
            primary: false,
            ..original.clone()
        };
        let new_primary = Monitor {
            device_path: Some("monitor-path-b".into()),
            ..original
        };
        let monitors = [new_primary, moved];
        let pinned = selected_monitor(&monitors, Some(&saved)).unwrap();
        let primary = selected_monitor(&monitors, None).unwrap();
        assert_eq!(pinned, &monitors[1]);
        assert_eq!(primary, &monitors[0]);
        assert_eq!(notch_origin(pinned.bounds, px(150.0), 0).x, px(-1035.0));
        assert_eq!(notch_origin(primary.bounds, px(150.0), 0).x, px(885.0));
    }
}
