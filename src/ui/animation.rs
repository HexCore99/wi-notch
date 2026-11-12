use std::time::Instant;

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

    pub fn sample(&self) -> (f32, f32, bool) {
        let progress = (self.start_time.elapsed().as_secs_f32() / 0.250).min(1.0);

        // let eased = 1.0 - (1.0 - progress).powi(3); // ease out cubic
        /*
         * smoothing function: 3p^2 - 2p^3 -> where p is progress
         * This is a smoothstep function that eases in and out.
         *
         */
        let eased = progress * progress * (3.0 - 2.0 * progress);

        let width = self.from.0 + (self.to.0 - self.from.0) * eased;
        let height = self.from.1 + (self.to.1 - self.from.1) * eased;
        (width, height, progress >= 1.0)
    }
}
