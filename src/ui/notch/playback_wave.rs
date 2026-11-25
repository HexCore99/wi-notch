use std::{
    f32::consts::TAU,
    time::{Duration, Instant},
};

use gpui::{prelude::*, *};

const LOOP_DURATION: Duration = Duration::from_secs(1);
const MAX_HEIGHTS: [f32; 5] = [7.0, 10.0, 13.0, 10.0, 7.0];
const SILENCE: [f32; 5] = [0.0; 5];
pub(super) const PEAK_INTERVAL: Duration = Duration::from_millis(65);

fn bar_heights(elapsed: Duration) -> [f32; 5] {
    let progress = (elapsed.as_secs_f64() / LOOP_DURATION.as_secs_f64()).fract() as f32;
    std::array::from_fn(|index| {
        let phase = TAU * (progress - index as f32 * 0.16);
        bar_height(index, (phase.sin() + 1.0) / 2.0)
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
        Self {
            heights: bar_heights(elapsed),
            animating: true,
        }
    }

    pub(super) fn bars(&self) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .children(self.heights.map(|height| {
                div()
                    .w(px(2.0))
                    .h(px(height))
                    .rounded_full()
                    .bg(rgb(0xffffff))
            }))
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
        let progress = self.sampled_at.map_or(1.0, |start| {
            now.saturating_duration_since(start).as_secs_f32() / PEAK_INTERVAL.as_secs_f32()
        });
        if progress >= 1.0 {
            return (self.target, false);
        }

        let levels = std::array::from_fn(|index| {
            let start = self.from[index];
            start + (self.target[index] - start) * progress
        });
        (levels, self.from != self.target)
    }

    pub(super) fn frame(&self, now: Instant) -> Option<WaveFrame> {
        let (levels, animating) = self.levels(now);
        (animating || levels != SILENCE).then(|| WaveFrame {
            heights: std::array::from_fn(|index| bar_height(index, levels[index])),
            animating,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{LOOP_DURATION, MAX_HEIGHTS, PEAK_INTERVAL, ReactiveWave, bar_heights};

    #[test]
    fn bars_change_height_and_repeat_each_cycle() {
        let start = bar_heights(Duration::ZERO);
        assert_eq!(start, bar_heights(LOOP_DURATION));
        assert_ne!(start, bar_heights(LOOP_DURATION / 4));

        for step in 0..16 {
            let heights = bar_heights(LOOP_DURATION * step / 16);
            for (height, max) in heights.into_iter().zip(MAX_HEIGHTS) {
                assert!((2.0..=max).contains(&height));
            }
        }
    }

    #[test]
    fn audio_peaks_move_across_the_bars_and_clear_after_silence() {
        let start = Instant::now();
        let mut wave = ReactiveWave::default();
        assert!(wave.frame(start).is_none());
        assert!(wave.push_peak(Some(0.0625), start));
        assert!(wave.push_peak(Some(0.25), start + PEAK_INTERVAL));

        let second_sample = start + PEAK_INTERVAL * 2;
        assert_eq!(wave.levels(second_sample).0, [0.5, 0.25, 0.0, 0.0, 0.0]);
        assert!(wave.frame(second_sample).is_some());

        for step in 2..=6 {
            wave.push_peak(None, start + PEAK_INTERVAL * step);
        }
        assert!(wave.frame(start + PEAK_INTERVAL * 7).is_none());
    }
}
