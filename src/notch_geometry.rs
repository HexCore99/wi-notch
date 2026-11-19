use gpui::{Bounds, Pixels, Point, point, px};

pub const EXPANDED_SIZE: (f32, f32) = (340.0, 162.0);
pub const TOP_GAP_STEP: u32 = 5;

pub fn max_top_gap(screen_height: Pixels) -> u32 {
    // Leave room for the size animation to overshoot the expanded height.
    let clearance = EXPANDED_SIZE.1 + 10.0;
    let available = (f32::from(screen_height) - clearance).max(0.0) as u32;
    available / TOP_GAP_STEP * TOP_GAP_STEP
}

pub fn notch_origin(screen: Bounds<Pixels>, width: Pixels, top_gap: u32) -> Point<Pixels> {
    let gap = top_gap.min(max_top_gap(screen.size.height));
    point(
        screen.origin.x + (screen.size.width - width) / 2.0,
        screen.origin.y + px(gap as f32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn notch_keeps_its_center_and_top_gap_as_it_expands() {
        let screen = Bounds::new(point(px(100.0), px(50.0)), size(px(700.0), px(350.0)));
        assert_eq!(
            notch_origin(screen, px(150.0), 50),
            point(px(375.0), px(100.0))
        );
        assert_eq!(
            notch_origin(screen, px(EXPANDED_SIZE.0), 50),
            point(px(280.0), px(100.0))
        );
    }

    #[test]
    fn top_gap_fits_the_expanded_notch_and_allows_zero() {
        let screen = Bounds::new(point(px(0.0), px(0.0)), size(px(700.0), px(350.0)));
        assert_eq!(max_top_gap(screen.size.height), 175);
        assert_eq!(max_top_gap(px(150.0)), 0);
        assert_eq!(notch_origin(screen, px(150.0), u32::MAX).y, px(175.0));
        assert_eq!(notch_origin(screen, px(150.0), 0).y, px(0.0));
    }
}
