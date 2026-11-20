use std::{path::PathBuf, time::Duration};

use rusqlite::{Connection, OpenFlags, params};

use super::{notifications::Toast, streams::read_local_image};

pub(super) const EDGE_STABLE_PACKAGE: &str = "Microsoft.MicrosoftEdge.Stable_8wekyb3d8bbwe!";

pub(super) fn open_history() -> Option<Connection> {
    let path = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
        .join("Microsoft/Windows/Notifications/wpndatabase.db");
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    db.busy_timeout(Duration::ZERO).ok()?;
    Some(db)
}

pub(super) fn find_payload(db: &Connection, toast: &Toast) -> Option<(String, Vec<u8>)> {
    // The listener can return Edge's main app ID for a toast stored under a site-specific ID.
    let edge_family = (toast.app_id == "MSEdge"
        || toast.app_id.strip_prefix(EDGE_STABLE_PACKAGE) == Some("App"))
    .then_some(EDGE_STABLE_PACKAGE);
    db.query_row(
        "SELECT h.PrimaryId, n.Payload
         FROM Notification n
         JOIN NotificationHandler h ON h.RecordId = n.HandlerId
         WHERE n.Id = ?1 AND n.ArrivalTime = ?2
           AND (h.PrimaryId = ?3 OR substr(h.PrimaryId, 1, length(?4)) = ?4)
           AND n.Type = 'toast' AND length(n.Payload) <= 131072",
        params![toast.key.id, toast.key.created, toast.app_id, edge_family],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .ok()
}

pub(super) fn read_artwork(db: &Connection, toast: &Toast) -> Option<Vec<u8>> {
    let (_, payload) = find_payload(db, toast)?;
    let document = roxmltree::Document::parse(std::str::from_utf8(&payload).ok()?).ok()?;
    let root = document.root_element();
    let visual = root.children().find(|node| node.has_tag_name("visual"))?;
    let binding = visual
        .children()
        .find(|node| node.has_tag_name("binding"))?;
    for placement in [Some("hero"), None, Some("appLogoOverride")] {
        if let Some(bytes) = binding
            .children()
            .filter(|node| node.has_tag_name("image") && node.attribute("placement") == placement)
            .filter_map(|node| node.attribute("src"))
            .find_map(|source| read_local_image(source, 4 * 1024 * 1024))
        {
            return Some(bytes);
        }
    }
    None
}
