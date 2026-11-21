use std::time::{Duration, Instant};

use gpui::{prelude::*, *};

use crate::windows::audio::{OutputVolume, Volume, VolumeKeyPresses};

pub(super) const VOLUME_HUD_SIZE: (f32, f32) = (320.0, 36.0);
const DISPLAY_TIME: Duration = Duration::from_millis(1500);
const FILL_TIME: Duration = Duration::from_millis(170);
const STRETCH_RISE: Duration = Duration::from_millis(65);
const STRETCH_RELEASE: Duration = Duration::from_millis(185);
const STRETCH_WIDTH: f32 = 12.0;

#[derive(Clone, Copy)]
pub(super) struct VolumeFrame {
    volume: Volume,
    fill: f32,
    pub(super) stretch: f32,
    pub(super) animating: bool,
}

struct FillAnimation {
    from: f32,
    to: f32,
    started: Instant,
}

impl FillAnimation {
    fn sample(&self, now: Instant) -> f32 {
        let progress =
            now.saturating_duration_since(self.started).as_secs_f32() / FILL_TIME.as_secs_f32();
        self.from + (self.to - self.from) * ease_out(progress)
    }
}

struct LimitStretch {
    direction: f32,
    from: f32,
    started: Instant,
}

impl LimitStretch {
    fn sample(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.started);
        if elapsed < STRETCH_RISE {
            let progress = elapsed.as_secs_f32() / STRETCH_RISE.as_secs_f32();
            self.from + (STRETCH_WIDTH - self.from) * ease_out(progress)
        } else {
            let progress = (elapsed - STRETCH_RISE).as_secs_f32() / STRETCH_RELEASE.as_secs_f32();
            STRETCH_WIDTH * (1.0 - ease_out(progress))
        }
    }
}

fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(3)
}

fn fill_for(volume: Volume) -> f32 {
    if volume.muted {
        0.0
    } else {
        f32::from(volume.percent) / 100.0
    }
}

#[derive(Default)]
pub(super) struct VolumeFeedback {
    last_reading: Option<OutputVolume>,
    hide_at: Option<Instant>,
    fill: Option<FillAnimation>,
    stretch: Option<LimitStretch>,
}

impl VolumeFeedback {
    fn visible(&self) -> Option<Volume> {
        self.hide_at?;
        self.last_reading.as_ref().map(|reading| reading.volume)
    }

    pub(super) fn frame(&self, now: Instant) -> Option<VolumeFrame> {
        let volume = self.visible()?;
        let stretch = self.stretch.as_ref();
        let fill = self.fill.as_ref();
        Some(VolumeFrame {
            volume,
            fill: fill.map_or_else(|| fill_for(volume), |fill| fill.sample(now)),
            stretch: stretch.map_or(0.0, |stretch| stretch.sample(now) * stretch.direction),
            animating: fill.is_some_and(|fill| now < fill.started + FILL_TIME)
                || stretch
                    .is_some_and(|pulse| now < pulse.started + STRETCH_RISE + STRETCH_RELEASE),
        })
    }

    pub(super) fn update(
        &mut self,
        reading: Option<OutputVolume>,
        now: Instant,
        presses: VolumeKeyPresses,
    ) -> bool {
        let was_visible = self.visible();
        let mut reshaped = false;
        match (&self.last_reading, &reading) {
            (Some(previous), Some(current)) if previous.device_id == current.device_id => {
                let level_changed = previous.volume.percent != current.volume.percent;
                let direction = match current.volume.percent {
                    0 if level_changed || presses.down => -1.0,
                    100 if level_changed || presses.up => 1.0,
                    _ => 0.0,
                };
                if direction != 0.0 {
                    let from = self
                        .stretch
                        .as_ref()
                        .filter(|pulse| pulse.direction == direction)
                        .map_or(0.0, |pulse| pulse.sample(now));
                    self.stretch = Some(LimitStretch {
                        direction,
                        from,
                        started: now,
                    });
                    reshaped = true;
                }
                if previous.volume != current.volume || reshaped {
                    let from = fill_for(previous.volume);
                    let to = fill_for(current.volume);
                    if from != to {
                        let from = self.fill.as_ref().map_or(from, |fill| fill.sample(now));
                        self.fill = Some(FillAnimation {
                            from,
                            to,
                            started: now,
                        });
                    }
                    self.hide_at = Some(now + DISPLAY_TIME);
                } else if self.hide_at.is_some_and(|deadline| now >= deadline) {
                    self.hide_at = None;
                }
            }
            _ => self.hide_at = None,
        }
        self.last_reading = reading;
        if self.hide_at.is_none() {
            self.fill = None;
            self.stretch = None;
        }
        reshaped || self.visible() != was_visible
    }
}

fn volume_icon(volume: Volume) -> &'static str {
    match (volume.muted, volume.percent) {
        (true, _) | (_, 0) => "\u{e74f}",
        (_, 1..=33) => "\u{e993}",
        (_, 34..=66) => "\u{e994}",
        _ => "\u{e995}",
    }
}

pub(super) fn volume_bar(frame: VolumeFrame) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(14.0))
        .child(
            div()
                .w(px(18.0))
                .flex_none()
                .font_family("Segoe MDL2 Assets")
                .font_weight(FontWeight::NORMAL)
                .text_size(px(16.0))
                .child(volume_icon(frame.volume)),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .h(px(6.0))
                .rounded_full()
                .overflow_hidden()
                .bg(rgb(0x454545))
                .child(
                    div()
                        .w(relative(frame.fill))
                        .h_full()
                        .rounded_full()
                        .bg(rgb(0xffffff)),
                ),
        )
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        DISPLAY_TIME, FILL_TIME, STRETCH_RELEASE, STRETCH_RISE, VolumeFeedback, volume_icon,
    };
    use crate::windows::audio::{OutputVolume, Volume, VolumeKeyPresses};

    const NO_KEYS: VolumeKeyPresses = VolumeKeyPresses {
        up: false,
        down: false,
    };

    fn reading(device: &str, percent: u8, muted: bool) -> Option<OutputVolume> {
        Some(OutputVolume {
            device_id: device.into(),
            volume: Volume { percent, muted },
        })
    }

    #[test]
    fn volume_and_mute_changes_restart_the_display_timer() {
        let mut feedback = VolumeFeedback::default();
        let start = Instant::now();
        assert!(!feedback.update(reading("speakers", 30, false), start, NO_KEYS));
        assert!(feedback.update(reading("speakers", 35, false), start, NO_KEYS));
        assert_eq!(feedback.visible().unwrap().percent, 35);

        let muted_at = start + Duration::from_secs(1);
        assert!(feedback.update(reading("speakers", 35, true), muted_at, NO_KEYS));
        assert!(feedback.visible().unwrap().muted);
        assert!(!feedback.update(reading("speakers", 35, true), start + DISPLAY_TIME, NO_KEYS));
        assert!(feedback.visible().is_some());
        assert!(feedback.update(
            reading("speakers", 35, true),
            muted_at + DISPLAY_TIME,
            NO_KEYS
        ));
        assert_eq!(feedback.visible(), None);
    }

    #[test]
    fn device_switches_and_reconnections_establish_a_new_baseline() {
        let mut feedback = VolumeFeedback::default();
        let now = Instant::now();
        assert!(!feedback.update(reading("speakers", 30, false), now, NO_KEYS));
        assert!(feedback.update(reading("speakers", 35, false), now, NO_KEYS));
        assert!(feedback.update(reading("headphones", 50, true), now, NO_KEYS));
        assert_eq!(feedback.visible(), None);
        assert!(!feedback.update(None, now, NO_KEYS));
        assert!(!feedback.update(reading("headphones", 25, false), now, NO_KEYS));
        assert_eq!(feedback.visible(), None);
        assert!(feedback.update(reading("headphones", 30, false), now, NO_KEYS));
    }

    #[test]
    fn changing_direction_retargets_the_fill_from_its_current_position() {
        let mut feedback = VolumeFeedback::default();
        let start = Instant::now();
        feedback.update(reading("speakers", 20, false), start, NO_KEYS);
        feedback.update(reading("speakers", 80, false), start, NO_KEYS);
        let halfway = start + FILL_TIME / 2;
        let rising = feedback.frame(halfway).unwrap().fill;
        assert!(rising > 0.2 && rising < 0.8);

        feedback.update(reading("speakers", 10, false), halfway, NO_KEYS);
        assert_eq!(feedback.frame(halfway).unwrap().fill, rising);
        let falling = feedback.frame(halfway + FILL_TIME / 2).unwrap().fill;
        assert!(falling > 0.1 && falling < rising);
        let settled = feedback.frame(halfway + FILL_TIME).unwrap();
        assert!((settled.fill - 0.1).abs() < 0.001);
        assert!(!settled.animating);
    }

    #[test]
    fn limits_stretch_outward_and_react_to_extra_volume_key_presses() {
        let mut feedback = VolumeFeedback::default();
        let start = Instant::now();
        feedback.update(reading("speakers", 98, false), start, NO_KEYS);
        feedback.update(reading("speakers", 100, false), start, NO_KEYS);
        assert!(feedback.frame(start + STRETCH_RISE).unwrap().stretch > 0.0);

        let repeated_at = start + STRETCH_RISE + STRETCH_RELEASE;
        assert_eq!(feedback.frame(repeated_at).unwrap().stretch, 0.0);
        assert!(feedback.update(
            reading("speakers", 100, false),
            repeated_at,
            VolumeKeyPresses {
                up: true,
                down: false
            },
        ));
        assert!(feedback.frame(repeated_at + STRETCH_RISE).unwrap().stretch > 0.0);

        let minimum_at = repeated_at + STRETCH_RISE + STRETCH_RELEASE;
        feedback.update(reading("speakers", 0, false), minimum_at, NO_KEYS);
        assert!(feedback.frame(minimum_at + STRETCH_RISE).unwrap().stretch < 0.0);
        let settled_at = minimum_at + STRETCH_RISE + STRETCH_RELEASE;
        assert_eq!(feedback.frame(settled_at).unwrap().stretch, 0.0);
    }

    #[test]
    fn speaker_icons_follow_the_volume_bands_and_mute_state() {
        let icon = |percent| {
            volume_icon(Volume {
                percent,
                muted: false,
            })
        };
        assert_ne!(icon(0), icon(1));
        assert_ne!(icon(33), icon(34));
        assert_ne!(icon(66), icon(67));
        assert_eq!(
            volume_icon(Volume {
                percent: 80,
                muted: true
            }),
            icon(0)
        );
    }
}
