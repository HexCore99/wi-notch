use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

pub(crate) struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let bytes: &[u8] = match path {
            "circle-half.svg" => include_bytes!("../assets/icons/circle-half.svg"),
            "display-fill.svg" => include_bytes!("../assets/icons/display-fill.svg"),
            _ => return Ok(None),
        };
        Ok(Some(Cow::Borrowed(bytes)))
    }

    fn list(&self, _: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
