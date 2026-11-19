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
        Foundation::{E_INVALIDARG, ERROR_INSUFFICIENT_BUFFER},
        Storage::Packaging::Appx::GetCurrentPackageFullName,
        UI::{
            Shell::{SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    },
    core::{Error, PCWSTR, w},
};

use super::streams::read_image_stream;

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

pub fn open_app(app_id: &str) -> ::windows::core::Result<()> {
    if app_id.is_empty() || app_id.contains('\0') {
        return Err(Error::new(E_INVALIDARG, "Invalid app ID"));
    }

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

    pub async fn read(&self) -> ::windows::core::Result<Snapshot> {
        let notifications = self
            .0
            .GetNotificationsAsync(NotificationKinds::Toast)?
            .await?;
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
