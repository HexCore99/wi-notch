use std::{
    f32::consts::TAU,
    time::{Duration, Instant},
};

use gpui::{prelude::*, *};

const LOOP_DURATION: Duration = Duration::from_secs(1);
const BOUNCE_DURATION: Duration = Duration::from_millis(675);
const ATTACK_TIME: Duration = Duration::from_millis(38);
const RELEASE_TIME: Duration = Duration::from_millis(105);
const MAX_HEIGHTS: [f32; 5] = [7.0, 10.0, 13.0, 10.0, 7.0];
const SILENCE: [f32; 5] = [0.0; 5];
pub(super) const PEAK_INTERVAL: Duration = Duration::from_millis(65);

fn wave_levels(elapsed: Duration, period: Duration) -> [f32; 5] {
    let progress = (elapsed.as_secs_f64() / period.as_secs_f64()).fract() as f32;
    std::array::from_fn(|index| {
        let phase = TAU * (progress - index as f32 * 0.16);
        (phase.sin() + 1.0) / 2.0
    })
}

fn bar_height(index: usize, level: f32) -> f32 {
    2.0 + (MAX_HEIGHTS[index] - 2.0) * level
}

pub(super) struct WaveFrame {
    heights: [f32; 5],
    pub(super) animating: bool,
}

impl WaveFrame {
    pub(super) fn looping(elapsed: Duration) -> Self {
        let levels = wave_levels(elapsed, LOOP_DURATION);
        Self {
            heights: std::array::from_fn(|index| bar_height(index, levels[index])),
            animating: true,
        }
    }

    pub(super) fn bars(&self) -> impl IntoElement + Styled {
        let heights = self.heights;
        // Paint the heights directly so layout rounding doesn't make the bars jump.
        canvas(
            |_, _, _| {},
            move |bounds, (), window, _| {
                let center_y = bounds.center().y;
                for (index, height) in heights.into_iter().enumerate() {
                    let height = px(height);
                    let left = bounds.origin.x + px(index as f32 * 4.0);
                    let bar =
                        Bounds::new(point(left, center_y - height / 2.0), size(px(2.0), height));
                    window.paint_quad(fill(bar, rgb(0xffffff)).corner_radii(px(1.0)));
                }
            },
        )
        .w(px(18.0))
    }
}

#[derive(Default)]
pub(super) struct ReactiveWave {
    from: [f32; 5],
    target: [f32; 5],
    sampled_at: Option<Instant>,
}

impl ReactiveWave {
    pub(super) fn push_peak(&mut self, peak: Option<f32>, now: Instant) -> bool {
        let level = peak
            .filter(|peak| peak.is_finite() && *peak >= 0.002)
            .unwrap_or_default()
            .min(1.0)
            .sqrt();
        let mut target = self.target;
        target.rotate_right(1);
        target[0] = level;
        if target == self.target {
            return false;
        }

        self.from = self.levels(now).0;
        self.target = target;
        self.sampled_at = Some(now);
        true
    }

    fn levels(&self, now: Instant) -> ([f32; 5], bool) {
        let Some(start) = self.sampled_at else {
            return (self.target, false);
        };
        let elapsed = now.saturating_duration_since(start).as_secs_f32();
        let rise = (-elapsed / ATTACK_TIME.as_secs_f32()).exp();
        let fall = (-elapsed / RELEASE_TIME.as_secs_f32()).exp();
        let levels = std::array::from_fn(|index| {
            let target = self.target[index];
            let difference = self.from[index] - target;
            let remaining = difference * if difference < 0.0 { rise } else { fall };
            if remaining.abs() <= 0.004 {
                target
            } else {
                target + remaining
            }
        });
        (levels, levels != self.target)
    }

    pub(super) fn frame(&self, now: Instant, elapsed: Duration) -> WaveFrame {
        let (levels, animating) = self.levels(now);
        let bounce = wave_levels(elapsed, BOUNCE_DURATION);
        WaveFrame {
            heights: std::array::from_fn(|index| {
                let strength = levels[index] * (0.35 + 0.65 * bounce[index]);
                bar_height(index, strength)
            }),
            animating: animating || levels != SILENCE,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        BOUNCE_DURATION, LOOP_DURATION, MAX_HEIGHTS, PEAK_INTERVAL, ReactiveWave, WaveFrame,
    };

    #[test]
    fn bars_change_height_and_repeat_each_cycle() {
        let start = WaveFrame::looping(Duration::ZERO).heights;
        assert_eq!(start, WaveFrame::looping(LOOP_DURATION).heights);
        assert_ne!(start, WaveFrame::looping(LOOP_DURATION / 4).heights);

        for step in 0..16 {
            let heights = WaveFrame::looping(LOOP_DURATION * step / 16).heights;
            for (height, max) in heights.into_iter().zip(MAX_HEIGHTS) {
                assert!((2.0..=max).contains(&height));
            }
        }
    }

    #[test]
    fn quiet_playback_shows_short_bars_before_and_after_peaks() {
        let start = Instant::now();
        let mut wave = ReactiveWave::default();
        let quiet = wave.frame(start, Duration::ZERO);
        assert_eq!(quiet.heights, [2.0; 5]);
        assert!(!quiet.animating);
        assert!(wave.push_peak(Some(0.0625), start));
        assert!(wave.push_peak(Some(0.25), start + PEAK_INTERVAL));

        let second_sample = start + PEAK_INTERVAL * 2;
        let levels = wave.levels(second_sample).0;
        assert!((0.25..0.5).contains(&levels[0]));
        assert!((0.0..0.25).contains(&levels[1]));
        assert!(wave.frame(second_sample, PEAK_INTERVAL * 2).animating);

        for step in 2..=6 {
            wave.push_peak(None, start + PEAK_INTERVAL * step);
        }
        let settled = Duration::from_secs(1);
        let quiet = wave.frame(start + settled, settled);
        assert_eq!(quiet.heights, [2.0; 5]);
        assert!(!quiet.animating);
    }

    #[test]
    fn steady_peaks_keep_the_bars_moving() {
        let start = Instant::now();
        let mut wave = ReactiveWave::default();
        for step in 0..5 {
            wave.push_peak(Some(0.64), start + PEAK_INTERVAL * step);
        }

        let settled = start + Duration::from_secs(1);
        assert!(!wave.push_peak(Some(0.64), settled));
        let first = wave.frame(settled, Duration::ZERO);
        let later = wave.frame(settled + BOUNCE_DURATION / 4, BOUNCE_DURATION / 4);
        assert!(first.animating && later.animating);
        assert_ne!(first.heights, later.heights);
    }

    #[test]
    fn peaks_rise_quickly_and_fall_gradually_from_the_current_height() {
        let start = Instant::now();
        let mut wave = ReactiveWave::default();
        wave.push_peak(Some(1.0), start);

        let change = start + PEAK_INTERVAL;
        let rising = wave.levels(change).0[0];
        assert!((0.5..1.0).contains(&rising));
        assert!(wave.push_peak(None, change));
        assert_eq!(wave.levels(change).0[0], rising);

        let falling = wave.levels(change + PEAK_INTERVAL).0[0];
        assert!(falling < rising && falling > rising / 2.0);
    }
}
