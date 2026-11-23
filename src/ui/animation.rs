use std::time::{Duration, Instant};

const PAGE_SWITCH_DURATION: Duration = Duration::from_millis(480);
const PAGE_SLIDE_DISTANCE: f32 = 6.0;

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

pub struct SizeAnimation {
    from: (f32, f32),
    to: (f32, f32),
    start_time: Instant,
}

impl SizeAnimation {
    pub fn new(from: (f32, f32), to: (f32, f32)) -> Self {
        Self {
            from,
            to,
            start_time: Instant::now(),
        }
    }

    pub fn min_size(&self) -> (f32, f32) {
        (self.from.0.min(self.to.0), self.from.1.min(self.to.1))
    }

    pub fn sample(&self) -> (f32, f32, bool) {
        let progress = (self.start_time.elapsed().as_secs_f32() / 0.520).min(1.0);

        // let eased = 1.0 - (1.0 - progress).powi(3); // ease out cubic
        /*
         * smoothing function: 3p^2 - 2p^3 -> where p is progress
         * This is a smoothstep function that eases in and out.
         *
         */
        // let eased = progress * progress * (3.0 - 2.0 * progress);
        let smooth = progress * progress * (3.0 - 2.0 * progress);

        let strength = 1.2;
        let shifted = smooth - 1.0;

        // Back easing: pass the target slightly, then settle back.
        let eased = 1.0 + (strength + 1.0) * shifted.powi(3) + strength * shifted.powi(2);

        let width = self.from.0 + (self.to.0 - self.from.0) * eased;
        let height = self.from.1 + (self.to.1 - self.from.1) * eased;
        (width, height, progress >= 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
