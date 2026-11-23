use std::borrow::Cow;

use gpui::{AssetSource, Div, SharedString, div, prelude::*, px, rgb, svg};

pub(crate) struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let bytes: &[u8] = match path {
            "circle-half.svg" => include_bytes!("../assets/icons/circle-half.svg"),
            "display-fill.svg" => include_bytes!("../assets/icons/display-fill.svg"),
            "tomato.svg" => include_bytes!("../assets/icons/tomato.svg"),
            "tomato-leaves.svg" => include_bytes!("../assets/icons/tomato-leaves.svg"),
            _ => return Ok(None),
        };
        Ok(Some(Cow::Borrowed(bytes)))
    }

    fn list(&self, _: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

pub(crate) fn tomato(size: f32) -> Div {
    // GPUI tints each SVG as a mask, so the fruit and leaves use separate layers.
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(
            svg()
                .path("tomato.svg")
                .size_full()
                .text_color(rgb(0xff5846)),
        )
        .child(
            svg()
                .path("tomato-leaves.svg")
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .size_full()
                .text_color(rgb(0x7cc34b)),
        )
}
