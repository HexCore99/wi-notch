use ::image::RgbaImage;
use ::windows::{
    Win32::{
        Foundation::SIZE,
        Graphics::Gdi::{
            BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS,
            DeleteDC, DeleteObject, GetDIBits, GetObjectW, HBITMAP,
        },
        System::{
            Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
            Registry::{HKEY_CLASSES_ROOT, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegGetValueW},
        },
        UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY},
    },
    core::{HSTRING, w},
};

use super::{notifications::browser_start_app_id, streams::read_local_image};

pub fn read_registered_icon(app_id: &str) -> Option<Vec<u8>> {
    if !valid_app_id(app_id) {
        return None;
    }

    // Desktop apps can register a logo for a notification identity of their own.
    let key = HSTRING::from(format!("AppUserModelId\\{app_id}"));
    let mut value = [0u16; 4096];
    let mut size = std::mem::size_of_val(&value) as u32;
    unsafe {
        RegGetValueW(
            HKEY_CLASSES_ROOT,
            &key,
            w!("IconUri"),
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .ok()
        .ok()?;
    }
    let end = value.iter().position(|unit| *unit == 0)?;
    let path = String::from_utf16(&value[..end]).ok()?;
    read_local_image(&path, 1024 * 1024)
}

pub fn read_shell_icon(app_id: &str) -> Option<RgbaImage> {
    if !valid_app_id(app_id) {
        return None;
    }
    let app_id = browser_start_app_id(app_id).unwrap_or(app_id);
    let path = HSTRING::from(format!("shell:AppsFolder\\{app_id}"));
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok()? };
    let icon = shell_bitmap(&path).and_then(|bitmap| {
        let pixels = bitmap_pixels(bitmap);
        unsafe {
            let _ = DeleteObject(bitmap.into());
        }
        pixels
    });
    unsafe { CoUninitialize() };
    icon
}

fn valid_app_id(id: &str) -> bool {
    !id.is_empty() && !id.contains(['\\', '/', ':', '\0'])
}

fn shell_bitmap(path: &HSTRING) -> Option<HBITMAP> {
    let item: IShellItemImageFactory = unsafe { SHCreateItemFromParsingName(path, None) }.ok()?;
    unsafe { item.GetImage(SIZE { cx: 64, cy: 64 }, SIIGBF_ICONONLY) }.ok()
}

fn bitmap_pixels(bitmap: HBITMAP) -> Option<RgbaImage> {
    let mut info = BITMAP::default();
    let size = std::mem::size_of::<BITMAP>() as i32;
    let read = unsafe { GetObjectW(bitmap.into(), size, Some((&mut info as *mut BITMAP).cast())) };
    let (width, height) = (info.bmWidth, info.bmHeight);
    if read != size || !(1..=256).contains(&width) || !(1..=256).contains(&height) {
        return None;
    }

    let mut format = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.is_invalid() {
        return None;
    }
    let rows = unsafe {
        let rows = GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut format,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        rows
    };
    if rows != height {
        return None;
    }

    let has_alpha = pixels.chunks_exact(4).any(|pixel| pixel[3] != 0);
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        if !has_alpha {
            pixel[3] = 255;
        }
    }
    RgbaImage::from_raw(width as u32, height as u32, pixels)
}
