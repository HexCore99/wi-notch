use std::time::{Duration, Instant};

const PAGE_SWITCH_DURATION: Duration = Duration::from_millis(480);
const PAGE_SLIDE_DISTANCE: f32 = 6.0;
const PANEL_TRANSITION_DURATION: Duration = Duration::from_millis(675);
const RESIZE_DURATION: Duration = Duration::from_millis(520);
const CLOSE_REBOUND: f32 = 0.72;

#[derive(Clone, Copy)]
pub(super) enum PageDirection {
    Left,
    Right,
}

pub(super) struct PageTransitionFrame {
    pub incoming: bool,
    pub opacity: f32,
    pub offset_x: f32,
}

pub(super) fn page_transition_frame(
    elapsed: Duration,
    direction: PageDirection,
) -> Option<PageTransitionFrame> {
    if elapsed >= PAGE_SWITCH_DURATION {
        return None;
    }

    let progress = elapsed.as_secs_f32() / PAGE_SWITCH_DURATION.as_secs_f32();
    let incoming = progress >= 0.5;
    let phase = if incoming {
        2.0 * progress - 1.0
    } else {
        1.0 - 2.0 * progress
    };
    let opacity = phase * phase * (3.0 - 2.0 * phase);
    let incoming_side = match direction {
        PageDirection::Left => -1.0,
        PageDirection::Right => 1.0,
    };
    let side = if incoming {
        incoming_side
    } else {
        -incoming_side
    };
    Some(PageTransitionFrame {
        incoming,
        opacity,
        offset_x: side * PAGE_SLIDE_DISTANCE * (1.0 - opacity),
    })
}

pub(super) enum SizeTransition {
    Expand,
    Shrink,
    Resize,
}

pub struct SizeAnimation {
    from: (f32, f32),
    to: (f32, f32),
    start_time: Instant,
    duration: Duration,
    rebound: Option<(f32, f32)>,
}

impl SizeAnimation {
    pub(super) fn new(from: (f32, f32), to: (f32, f32), transition: SizeTransition) -> Self {
        let (duration, rebound) = match transition {
            SizeTransition::Expand => (PANEL_TRANSITION_DURATION, None),
            SizeTransition::Resize => (RESIZE_DURATION, None),
            SizeTransition::Shrink => {
                // Keep the smallest notch large enough to remain visible during the bounce.
                let width = to.0 - ((from.0 - to.0) * 0.12).clamp(0.0, 20.0_f32.min(to.0 * 0.25));
                let height = to.1 - ((from.1 - to.1) * 0.12).clamp(0.0, 8.0_f32.min(to.1 * 0.25));
                (PANEL_TRANSITION_DURATION, Some((width, height)))
            }
        };
        Self {
            from,
            to,
            start_time: Instant::now(),
            duration,
            rebound,
        }
    }

    pub fn min_size(&self) -> (f32, f32) {
        let smallest = self.rebound.unwrap_or(self.to);
        (self.from.0.min(smallest.0), self.from.1.min(smallest.1))
    }

    pub fn sample(&self) -> (f32, f32, bool) {
        self.sample_at(self.start_time.elapsed())
    }

    fn sample_at(&self, elapsed: Duration) -> (f32, f32, bool) {
        if elapsed >= self.duration {
            return (self.to.0, self.to.1, true);
        }
        let progress = elapsed.as_secs_f32() / self.duration.as_secs_f32();

        let (from, to, eased) = if let Some(rebound) = self.rebound {
            if progress < CLOSE_REBOUND {
                (self.from, rebound, smoothstep(progress / CLOSE_REBOUND))
            } else {
                let settle = (progress - CLOSE_REBOUND) / (1.0 - CLOSE_REBOUND);
                (rebound, self.to, smoothstep(settle))
            }
        } else {
            let shifted = smoothstep(progress) - 1.0;
            let eased = 1.0 + 2.2 * shifted.powi(3) + 1.2 * shifted.powi(2);
            (self.from, self.to, eased)
        };

        let width = from.0 + (to.0 - from.0) * eased;
        let height = from.1 + (to.1 - from.1) * eased;
        (width, height, false)
    }
}

fn smoothstep(progress: f32) -> f32 {
    progress * progress * (3.0 - 2.0 * progress)
}

pub(super) fn closing_panel_opacity(expansion: f32) -> f32 {
    smoothstep(((expansion - 0.55) / 0.45).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinking_bounces_below_the_target_then_settles() {
        let expanded = (340.0, 162.0);
        for (target, smallest) in [((150.0, 35.0), (130.0, 27.0)), ((64.0, 8.0), (48.0, 6.0))] {
            let closing = SizeAnimation::new(expanded, target, SizeTransition::Shrink);
            assert_eq!(closing.sample_at(Duration::ZERO), (340.0, 162.0, false));
            assert_eq!(closing.min_size(), smallest);

            let (width, height, finished) =
                closing.sample_at(PANEL_TRANSITION_DURATION.mul_f32(CLOSE_REBOUND));
            assert!((width - smallest.0).abs() < 0.01);
            assert!((height - smallest.1).abs() < 0.01);
            assert!(!finished);

            let settling = PANEL_TRANSITION_DURATION.mul_f32((CLOSE_REBOUND + 1.0) / 2.0);
            let (width, height, _) = closing.sample_at(settling);
            assert!(width > smallest.0 && width < target.0);
            assert!(height > smallest.1 && height < target.1);
            assert_eq!(
                closing.sample_at(PANEL_TRANSITION_DURATION),
                (target.0, target.1, true)
            );
        }

        let opening = SizeAnimation::new((150.0, 35.0), expanded, SizeTransition::Expand);
        assert_eq!(
            opening.sample_at(PANEL_TRANSITION_DURATION),
            (340.0, 162.0, true)
        );
    }

    #[test]
    fn closing_panel_fades_before_the_rebound() {
        assert_eq!(closing_panel_opacity(1.0), 1.0);
        let just_started = closing_panel_opacity(0.98);
        let halfway = closing_panel_opacity(0.775);
        assert!(just_started > halfway && just_started < 1.0);
        assert!((halfway - 0.5).abs() < 0.001);
        assert_eq!(closing_panel_opacity(0.55), 0.0);
        assert_eq!(closing_panel_opacity(0.0), 0.0);
    }

    #[test]
    fn page_switch_fades_and_slides_in_the_chevron_direction() {
        for (direction, sign) in [(PageDirection::Right, 1.0), (PageDirection::Left, -1.0)] {
            for (ms, incoming, opacity, right_offset) in [
                (0, false, 1.0, 0.0),
                (120, false, 0.5, -3.0),
                (240, true, 0.0, 6.0),
                (360, true, 0.5, 3.0),
            ] {
                let frame = page_transition_frame(Duration::from_millis(ms), direction).unwrap();
                assert_eq!(frame.incoming, incoming);
                assert!((frame.opacity - opacity).abs() < 0.001);
                assert!((frame.offset_x - right_offset * sign).abs() < 0.001);
            }
            let last_frame = page_transition_frame(Duration::from_millis(479), direction).unwrap();
            assert!(last_frame.opacity > 0.999);
            assert!(last_frame.offset_x.abs() < 0.001);
            assert!(page_transition_frame(Duration::from_millis(480), direction).is_none());
        }
    }
}
