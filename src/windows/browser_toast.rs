use super::{
    notification_history::{EDGE_STABLE_PACKAGE, find_payload, open_history},
    notifications::Toast,
};
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
    core::{GUID, HSTRING, PCWSTR, w},
};

// Edge Stable declares this activator for its notification helper.
const EDGE_STABLE_ACTIVATOR: GUID = GUID::from_u128(0x628ace20_b77a_456f_a88d_547db6ceedd5);

pub(super) fn is_edge(app_id: &str) -> bool {
    app_id == "MSEdge" || app_id.starts_with(EDGE_STABLE_PACKAGE)
}

pub(super) fn activate(toast: &Toast) -> Option<()> {
    let (app_id, payload) = find_payload(&open_history()?, toast)?;
    let xml = std::str::from_utf8(&payload).ok()?;
    let document = roxmltree::Document::parse(xml).ok()?;
    let args = launch_arguments(document.root_element(), is_edge(&app_id))?;

    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok()? };
    let result = send_to_browser(&app_id, args);
    unsafe { CoUninitialize() };
    result
}

fn launch_arguments<'a>(root: roxmltree::Node<'a, '_>, edge: bool) -> Option<&'a str> {
    if !root.has_tag_name("toast")
        || !matches!(root.attribute("activationType"), None | Some("foreground"))
    {
        return None;
    }
    let args = root.attribute("launch").filter(|args| !args.is_empty())?;
    if edge {
        return Some(args);
    }
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

fn browser_activator(app_id: &str) -> Option<INotificationActivationCallback> {
    let clsid = if is_edge(app_id) {
        EDGE_STABLE_ACTIVATOR
    } else {
        registered_activator(app_id)?
    };
    unsafe { CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER | CLSCTX_NO_CODE_DOWNLOAD) }.ok()
}

fn registered_activator(app_id: &str) -> Option<GUID> {
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
        CLSIDFromString(PCWSTR(value.as_ptr())).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::{EDGE_STABLE_PACKAGE, find_payload, is_edge, launch_arguments};
    use crate::windows::notifications::{Toast, ToastKey};
    use rusqlite::{Connection, params};

    #[test]
    fn browser_body_action_preserves_the_original_arguments() {
        let xml = r#"<toast launch="program&#10;zen&#10;windowsTag&#10;example"><actions>
            <action arguments="different-button-action"/>
        </actions></toast>"#;
        let document = roxmltree::Document::parse(xml).unwrap();
        assert_eq!(
            launch_arguments(document.root_element(), false),
            Some("program\nzen\nwindowsTag\nexample")
        );

        for xml in [
            r#"<toast launch="skipNotificationServer&#10;true"/>"#,
            r#"<toast activationType="protocol" launch="windowsTag&#10;example"/>"#,
            r#"<toast launch="program&#10;windowsTag&#10;other&#10;value"/>"#,
        ] {
            let document = roxmltree::Document::parse(xml).unwrap();
            assert_eq!(launch_arguments(document.root_element(), false), None);
        }
    }

    #[test]
    fn edge_site_notifications_keep_the_opaque_body_action() {
        assert!(is_edge(
            "Microsoft.MicrosoftEdge.Stable_8wekyb3d8bbwe!https://example.com/"
        ));
        assert!(!is_edge("OtherApp!https://example.com/"));

        let xml = r#"<toast launch="origin|0|Default|action&amp;data"/>"#;
        let document = roxmltree::Document::parse(xml).unwrap();
        assert_eq!(
            launch_arguments(document.root_element(), true),
            Some("origin|0|Default|action&data")
        );
        assert_eq!(launch_arguments(document.root_element(), false), None);
    }

    #[test]
    fn edge_main_app_id_resolves_only_the_matching_site_toast() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE NotificationHandler (RecordId INTEGER, PrimaryId TEXT);
             CREATE TABLE Notification (Id INTEGER, ArrivalTime INTEGER, HandlerId INTEGER,
                                        Type TEXT, Payload BLOB);
             INSERT INTO Notification VALUES (7, 42, 1, 'toast', x'01');",
        )
        .unwrap();
        let site_id = format!("{EDGE_STABLE_PACKAGE}https://example.com/");
        db.execute(
            "INSERT INTO NotificationHandler VALUES (1, ?1)",
            params![site_id],
        )
        .unwrap();
        let mut toast = Toast {
            key: ToastKey { id: 7, created: 42 },
            app_id: format!("{EDGE_STABLE_PACKAGE}App"),
            app_name: String::new(),
            title: String::new(),
            body: String::new(),
        };
        assert_eq!(find_payload(&db, &toast), Some((site_id, vec![1])));
        toast.key.created -= 1;
        assert_eq!(find_payload(&db, &toast), None);
        toast.key.created += 1;
        toast.app_id = "OtherBrowser".into();
        assert_eq!(find_payload(&db, &toast), None);
    }
}
