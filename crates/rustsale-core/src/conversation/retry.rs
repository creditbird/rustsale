use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 5 }
    }
}

impl RetryPolicy {
    pub fn delay(&self, attempt: u32, jitter_unit: f64) -> Duration {
        let base_secs = match attempt {
            1 => 5.0,
            2 => 15.0,
            3 => 45.0,
            4 => 120.0,
            _ => 300.0,
        };

        let clamped_jitter = jitter_unit.clamp(0.0, 1.0);
        let multiplier = 0.8 + 0.4 * clamped_jitter;
        let final_secs = base_secs * multiplier;
        Duration::from_secs_f64(final_secs.round())
    }

    pub fn should_stop(&self, attempt: u32) -> bool {
        attempt >= self.max_attempts
    }
}
