use std::{path::PathBuf, time::Duration};

use ::windows::{
    Win32::{
        System::{
            Com::{
                CLSCTX_LOCAL_SERVER, CLSCTX_NO_CODE_DOWNLOAD, CLSIDFromString,
                COINIT_MULTITHREADED, CoAllowSetForegroundWindow, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            Registry::{HKEY_CLASSES_ROOT, RRF_RT_REG_SZ, RegGetValueW},
        },
        UI::Notifications::INotificationActivationCallback,
    },
    core::{HSTRING, PCWSTR, w},
};
use rusqlite::{Connection, OpenFlags, params};

use super::notifications::Toast;

pub(super) fn activate(toast: &Toast) -> Option<()> {
    let payload = read_payload(toast)?;
    let xml = std::str::from_utf8(&payload).ok()?;
    let document = roxmltree::Document::parse(xml).ok()?;
    let args = launch_arguments(document.root_element())?;

    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok()? };
    let result = send_to_browser(&toast.app_id, args);
    unsafe { CoUninitialize() };
    result
}

fn launch_arguments<'a>(root: roxmltree::Node<'a, '_>) -> Option<&'a str> {
    if !root.has_tag_name("toast")
        || !matches!(root.attribute("activationType"), None | Some("foreground"))
    {
        return None;
    }
    let args = root.attribute("launch")?;
    let mut fields = args.lines();
    while let Some((key, value)) = fields.next().zip(fields.next()) {
        if key == "windowsTag" && !value.is_empty() {
            return Some(args);
        }
    }
    None
}

fn send_to_browser(app_id: &str, args: &str) -> Option<()> {
    let activator = browser_activator(app_id)?;
    unsafe {
        let _ = CoAllowSetForegroundWindow(&activator, None);
        activator.Activate(&HSTRING::from(app_id), &HSTRING::from(args), &[])
    }
    .ok()
}

fn read_payload(toast: &Toast) -> Option<Vec<u8>> {
    // UserNotificationListener gives us the displayed content. The Windows
    // history database also holds the original toast body activation arguments.
    let path = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
        .join("Microsoft/Windows/Notifications/wpndatabase.db");
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    db.busy_timeout(Duration::ZERO).ok()?;
    db.query_row(
        "SELECT n.Payload
         FROM Notification n
         JOIN NotificationHandler h ON h.RecordId = n.HandlerId
         WHERE n.Id = ?1 AND n.ArrivalTime = ?2 AND h.PrimaryId = ?3
           AND n.Type = 'toast' AND length(n.Payload) <= 131072",
        params![toast.key.id, toast.key.created, toast.app_id],
        |row| row.get(0),
    )
    .ok()
}

fn browser_activator(app_id: &str) -> Option<INotificationActivationCallback> {
    let key = HSTRING::from(format!("AppUserModelId\\{app_id}"));
    // A braced GUID plus its trailing null, as stored by the browser installer.
    let mut value = [0u16; 39];
    let mut value_bytes = std::mem::size_of_val(&value) as u32;
    unsafe {
        RegGetValueW(
            HKEY_CLASSES_ROOT,
            &key,
            w!("CustomActivator"),
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut value_bytes),
        )
        .ok()
        .ok()?;
        let clsid = CLSIDFromString(PCWSTR(value.as_ptr())).ok()?;
        CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER | CLSCTX_NO_CODE_DOWNLOAD).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::launch_arguments;

    #[test]
    fn browser_body_action_preserves_the_original_arguments() {
        let xml = r#"<toast launch="program&#10;zen&#10;windowsTag&#10;example"><actions>
            <action arguments="different-button-action"/>
        </actions></toast>"#;
        let document = roxmltree::Document::parse(xml).unwrap();
        assert_eq!(
            launch_arguments(document.root_element()),
            Some("program\nzen\nwindowsTag\nexample")
        );

        for xml in [
            r#"<toast launch="skipNotificationServer&#10;true"/>"#,
            r#"<toast activationType="protocol" launch="windowsTag&#10;example"/>"#,
            r#"<toast launch="program&#10;windowsTag&#10;other&#10;value"/>"#,
        ] {
            let document = roxmltree::Document::parse(xml).unwrap();
            assert_eq!(launch_arguments(document.root_element()), None);
        }
    }
}
