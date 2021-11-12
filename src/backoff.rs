use rand::random;
use std::time::Duration;

pub struct Backoff {
    pub min: Duration,
    pub max: Duration,
    pub current_delay: Duration,
}

impl Backoff {
    pub fn new(min: Duration, max: Duration) -> Self {
        Backoff {
            min,
            max,
            current_delay: min,
        }
    }

    pub fn reset(&mut self) {
        self.current_delay = self.min;
    }

    pub fn next(&mut self) -> Duration {
        self.current_delay =
            (self.current_delay + self.current_delay.mul_f32(2.0 * random::<f32>())).min(self.max);
        self.current_delay
    }
}
