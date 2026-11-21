use gpui::{Bounds, Pixels, Point, point, px};

pub const EXPANDED_SIZE: (f32, f32) = (340.0, 162.0);
pub const COLLAPSED_SIZE: (f32, f32) = (64.0, 8.0);

pub fn max_top_gap(screen_height: Pixels) -> u32 {
    // Leave room for the size animation to overshoot the expanded height.
    let clearance = EXPANDED_SIZE.1 + 10.0;
    (f32::from(screen_height) - clearance).max(0.0) as u32
}

pub fn notch_origin(screen: Bounds<Pixels>, width: Pixels, top_gap: u32) -> Point<Pixels> {
    notch_origin_for_height(screen, width, px(EXPANDED_SIZE.1), top_gap)
}

pub fn notch_origin_for_height(
    screen: Bounds<Pixels>,
    width: Pixels,
    height: Pixels,
    top_gap: u32,
) -> Point<Pixels> {
    let available = f32::from(screen.size.height - height).max(0.0) as u32;
    let gap = top_gap.min(max_top_gap(screen.size.height)).min(available);
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
        assert_eq!(max_top_gap(screen.size.height), 178);
        assert_eq!(max_top_gap(px(150.0)), 0);
        assert_eq!(notch_origin(screen, px(150.0), u32::MAX).y, px(178.0));
        assert_eq!(notch_origin(screen, px(150.0), 0).y, px(0.0));
    }

    #[test]
    fn tall_notification_panel_shifts_up_only_when_needed() {
        let screen = Bounds::new(point(px(0.0), px(30.0)), size(px(700.0), px(600.0)));
        let panel = notch_origin_for_height(screen, px(340.0), px(330.0), 400);
        assert_eq!(panel, point(px(180.0), px(300.0)));
        assert_eq!(notch_origin(screen, px(150.0), 400).y, px(430.0));
    }
}
