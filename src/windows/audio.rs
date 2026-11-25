use std::sync::atomic::{AtomicU8, Ordering};

use ::windows::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    Media::Audio::{
        Endpoints::{IAudioEndpointVolume, IAudioMeterInformation},
        IMMDeviceEnumerator, MMDeviceEnumerator, PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT,
        SND_NOSTOP, SND_SYSTEM, eMultimedia, eRender,
    },
    System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoTaskMemFree, CoUninitialize,
        },
        LibraryLoader::GetModuleHandleW,
    },
    UI::{
        Input::KeyboardAndMouse::{VK_VOLUME_DOWN, VK_VOLUME_UP},
        WindowsAndMessaging::{
            CallNextHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, SetWindowsHookExW,
            UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
        },
    },
};
use ::windows::core::PCWSTR;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Volume {
    pub percent: u8,
    pub muted: bool,
}

pub struct OutputVolume {
    pub device_id: String,
    pub volume: Volume,
    pub peak: Option<f32>,
}

const VOLUME_UP: u8 = 1;
const VOLUME_DOWN: u8 = 2;
static PRESSED_VOLUME_KEYS: AtomicU8 = AtomicU8::new(0);

pub enum AlertSound {
    Notification,
    Prayer,
    Pomodoro,
}

pub fn play_alert_sound(sound: AlertSound) {
    // The embedded bytes stay valid for the duration of asynchronous playback.
    let wav: &'static [u8] = match sound {
        AlertSound::Notification => include_bytes!("../../assets/sounds/notification.wav"),
        AlertSound::Prayer => include_bytes!("../../assets/sounds/prayer.wav"),
        AlertSound::Pomodoro => include_bytes!("../../assets/sounds/pomodoro.wav"),
    };
    let flags = SND_MEMORY | SND_ASYNC | SND_NODEFAULT | SND_SYSTEM;
    let flags = if matches!(sound, AlertSound::Notification) {
        flags | SND_NOSTOP
    } else {
        flags
    };
    unsafe {
        let _ = PlaySoundW(PCWSTR(wav.as_ptr().cast()), None, flags);
    }
}

#[derive(Default)]
pub struct VolumeKeyPresses {
    pub up: bool,
    pub down: bool,
}

pub struct VolumeKeyWatch(HHOOK);

impl VolumeKeyWatch {
    pub fn start() -> ::windows::core::Result<Self> {
        let module = unsafe { GetModuleHandleW(None) }?;
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_volume_key), Some(module.into()), 0) }
            .map(Self)
    }

    pub fn take_presses(&self) -> VolumeKeyPresses {
        let keys = PRESSED_VOLUME_KEYS.swap(0, Ordering::Relaxed);
        VolumeKeyPresses {
            up: keys & VOLUME_UP != 0,
            down: keys & VOLUME_DOWN != 0,
        }
    }
}

impl Drop for VolumeKeyWatch {
    fn drop(&mut self) {
        let _ = unsafe { UnhookWindowsHookEx(self.0) };
        PRESSED_VOLUME_KEYS.store(0, Ordering::Relaxed);
    }
}

unsafe extern "system" fn on_volume_key(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && matches!(message.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
        let key = unsafe { &*(data.0 as *const KBDLLHOOKSTRUCT) };
        let flag = match key.vkCode {
            code if code == u32::from(VK_VOLUME_UP.0) => VOLUME_UP,
            code if code == u32::from(VK_VOLUME_DOWN.0) => VOLUME_DOWN,
            _ => 0,
        };
        if flag != 0 {
            // Windows handles the key. This only lets the HUD react at an unchanged limit.
            PRESSED_VOLUME_KEYS.fetch_or(flag, Ordering::Relaxed);
        }
    }
    unsafe { CallNextHookEx(None, code, message, data) }
}

pub fn read_default_output(read_peak: bool) -> Option<OutputVolume> {
    // The background task reads the endpoint in the same COM apartment that opens it.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok()? };
    let result = read_endpoint(read_peak);
    unsafe { CoUninitialize() };
    result
}

fn read_endpoint(read_peak: bool) -> Option<OutputVolume> {
    let devices: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) }.ok()?;
    let device = unsafe { devices.GetDefaultAudioEndpoint(eRender, eMultimedia) }.ok()?;
    let id = unsafe { device.GetId() }.ok()?;
    let device_id = unsafe { id.to_string() };
    unsafe { CoTaskMemFree(Some(id.0.cast())) };
    let device_id = device_id.ok()?;

    let endpoint: IAudioEndpointVolume =
        unsafe { device.Activate(CLSCTX_INPROC_SERVER, None) }.ok()?;
    let level = unsafe { endpoint.GetMasterVolumeLevelScalar() }.ok()?;
    let muted = unsafe { endpoint.GetMute() }.ok()?.as_bool();
    // Endpoint peaks are measured before applying volume and mute.
    let peak = if read_peak && !muted && level > 0.0 {
        unsafe { device.Activate::<IAudioMeterInformation>(CLSCTX_INPROC_SERVER, None) }
            .ok()
            .and_then(|meter| unsafe { meter.GetPeakValue() }.ok())
    } else {
        None
    };
    Some(OutputVolume {
        device_id,
        volume: Volume {
            percent: (level.clamp(0.0, 1.0) * 100.0).round() as u8,
            muted,
        },
        peak,
    })
}
