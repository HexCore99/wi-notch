use std::{cmp::Reverse, collections::HashMap};

use ::windows::{
    ApplicationModel::AppDisplayInfo,
    Foundation::Size,
    UI::Notifications::{
        KnownNotificationBindings,
        Management::{
            UserNotificationListener, UserNotificationListenerAccessStatus as AccessStatus,
        },
        NotificationKinds, UserNotification,
    },
    Win32::{
        Foundation::{E_INVALIDARG, ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER},
        Storage::Packaging::Appx::GetCurrentPackageFullName,
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
        UI::{
            Shell::{SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    },
    core::{Error, HRESULT, PCWSTR, w},
};

use super::{browser_toast, streams::read_image_stream, window::focus_app_window};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Unpackaged,
    Unavailable,
    Unspecified,
    Denied,
    Allowed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToastKey {
    pub id: u32,
    pub created: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub key: ToastKey,
    pub app_id: String,
    pub app_name: String,
    pub title: String,
    pub body: String,
}

#[derive(Default)]
pub struct Snapshot {
    pub toasts: Vec<Toast>,
    pub apps: HashMap<String, AppDisplayInfo>,
}

pub struct Listener(UserNotificationListener);

pub async fn open_notification(
    toast: Toast,
    executor: &gpui::BackgroundExecutor,
) -> ::windows::core::Result<()> {
    let app_id = toast.app_id.clone();
    if app_id.is_empty() || app_id.contains('\0') {
        return Err(Error::new(E_INVALIDARG, "Invalid app ID"));
    }

    let browser_id = browser_start_app_id(&app_id);
    if let Some(start_id) = browser_id {
        if Listener::connect().is_ok_and(|listener| listener.access() == Access::Allowed)
            && executor
                .spawn(async move { browser_toast::activate(&toast).is_some() })
                .await
        {
            return Ok(());
        }
        if focus_app_window(start_id)? {
            return Ok(());
        }
    }

    match open_start_app(&app_id) {
        Err(error) if error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => {
            if let Some(start_id) = browser_id {
                open_start_app(start_id)
            } else {
                Err(error)
            }
        }
        result => result,
    }
}

fn browser_start_app_id(app_id: &str) -> Option<&str> {
    // These desktop browsers use a separate toast ID built from their Start menu ID.
    let install_hash = app_id
        .strip_prefix("ZenToast-")
        .or_else(|| app_id.strip_prefix("FirefoxToast-"))?;
    (!install_hash.is_empty()
        && install_hash.len() <= 16
        && install_hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then_some(install_hash)
}

fn open_start_app(app_id: &str) -> ::windows::core::Result<()> {
    let target: Vec<u16> = format!("shell:AppsFolder\\{app_id}")
        .encode_utf16()
        .chain([0])
        .collect();
    let mut launch = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_FLAG_NO_UI,
        lpVerb: w!("open"),
        lpFile: PCWSTR(target.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut launch) }
}

impl Listener {
    pub fn connect() -> Result<Self, Access> {
        let mut length = 0;
        if unsafe { GetCurrentPackageFullName(&mut length, None) } != ERROR_INSUFFICIENT_BUFFER {
            return Err(Access::Unpackaged);
        }

        UserNotificationListener::Current()
            .map(Self)
            .map_err(|_| Access::Unavailable)
    }

    pub fn access(&self) -> Access {
        match self.0.GetAccessStatus() {
            Ok(AccessStatus::Allowed) => Access::Allowed,
            Ok(AccessStatus::Denied) => Access::Denied,
            Ok(AccessStatus::Unspecified) => Access::Unspecified,
            _ => Access::Unavailable,
        }
    }

    // Call this from the GPUI foreground task started by the Enable button.
    pub async fn request_access(&self) -> Access {
        let Ok(request) = self.0.RequestAccessAsync() else {
            return Access::Unavailable;
        };
        match request.await {
            Ok(AccessStatus::Allowed) => Access::Allowed,
            Ok(AccessStatus::Denied) => Access::Denied,
            Ok(AccessStatus::Unspecified) => Access::Unspecified,
            _ => Access::Unavailable,
        }
    }

    pub fn poll() -> (Access, Option<Snapshot>) {
        // Create the listener in the same COM apartment that reads from it.
        if unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.is_err() {
            return (Access::Unavailable, None);
        }

        let snapshot = match Self::connect() {
            Ok(listener) => listener.read_snapshot(),
            Err(access) => (access, None),
        };
        unsafe { CoUninitialize() };
        snapshot
    }

    fn read_snapshot(&self) -> (Access, Option<Snapshot>) {
        let mut access = self.access();
        let snapshot = if access == Access::Allowed {
            let snapshot = self.read().ok();
            // Access can be revoked while waiting for Windows.
            access = self.access();
            snapshot.filter(|_| access == Access::Allowed)
        } else {
            None
        };

        (access, snapshot)
    }

    fn read(&self) -> ::windows::core::Result<Snapshot> {
        let notifications = self
            .0
            .GetNotificationsAsync(NotificationKinds::Toast)?
            .get()?;
        let mut snapshot = Snapshot::default();
        for index in 0..notifications.Size()? {
            if let Ok(notification) = notifications.GetAt(index)
                && let Some(toast) = read_toast(&notification, &mut snapshot.apps)
            {
                snapshot.toasts.push(toast);
            }
        }
        snapshot
            .toasts
            .sort_unstable_by_key(|toast| Reverse((toast.key.created, toast.key.id)));
        snapshot.toasts.dedup_by_key(|toast| toast.key);
        Ok(snapshot)
    }
}

fn read_toast(
    notification: &UserNotification,
    apps: &mut HashMap<String, AppDisplayInfo>,
) -> Option<Toast> {
    let key = ToastKey {
        id: notification.Id().ok()?,
        created: notification.CreationTime().ok()?.UniversalTime,
    };
    let app = notification.AppInfo().ok();
    let app_id = app
        .as_ref()
        .and_then(|app| app.AppUserModelId().ok())
        .unwrap_or_default()
        .to_string();
    let display = app.and_then(|app| app.DisplayInfo().ok());
    let app_name = display
        .as_ref()
        .and_then(|display| display.DisplayName().ok())
        .map(|name| name.to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Windows".into());
    if let Some(display) = display
        && !app_id.is_empty()
    {
        apps.entry(app_id.clone()).or_insert(display);
    }

    let lines = toast_text(notification).unwrap_or_default();
    let mut lines = lines.into_iter();
    let title = lines.next().unwrap_or_else(|| app_name.clone());
    let body = lines.collect::<Vec<_>>().join(" ");
    Some(Toast {
        key,
        app_id,
        app_name,
        title,
        body,
    })
}

fn toast_text(notification: &UserNotification) -> Option<Vec<String>> {
    let binding = notification
        .Notification()
        .ok()?
        .Visual()
        .ok()?
        .GetBinding(&KnownNotificationBindings::ToastGeneric().ok()?)
        .ok()?;
    let elements = binding.GetTextElements().ok()?;
    Some(
        (0..elements.Size().ok()?)
            .filter_map(|index| elements.GetAt(index).ok()?.Text().ok())
            .map(|text| {
                text.to_string()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|text| !text.is_empty())
            .collect(),
    )
}

pub async fn read_icon(app: AppDisplayInfo) -> Option<Vec<u8>> {
    let logo = app
        .GetLogo(Size {
            Width: 32.0,
            Height: 32.0,
        })
        .ok()?;
    let stream = logo.OpenReadAsync().ok()?.await.ok()?;
    let (_, bytes) = read_image_stream(stream, 1024 * 1024).await?;
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::browser_start_app_id;

    #[test]
    fn only_recognized_browser_toast_ids_map_to_start_entries() {
        assert_eq!(
            browser_start_app_id("ZenToast-F0DC299D809B9700"),
            Some("F0DC299D809B9700")
        );
        assert_eq!(
            browser_start_app_id("FirefoxToast-308046B0AF4A39CB"),
            Some("308046B0AF4A39CB")
        );
        for app_id in [
            "Microsoft.WindowsNotepad_8wekyb3d8bbwe!App",
            "OtherToast-F0DC299D809B9700",
            "ZenToast-",
            "ZenToast-not-a-hash",
            "ZenToast-0123456789ABCDEF0",
        ] {
            assert_eq!(browser_start_app_id(app_id), None);
        }
    }
}
