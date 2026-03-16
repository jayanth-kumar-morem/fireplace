use rand::Rng;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ReconnectState {
    attempts: u32,
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
    jitter_max_ms: u64,
}

impl ReconnectState {
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            attempts: 0,
            max_attempts,
            base_delay,
            max_delay,
            jitter_max_ms: 1_000,
        }
    }

    pub fn new_with_jitter(
        max_attempts: u32,
        base_delay: Duration,
        max_delay: Duration,
        jitter_max_ms: u64,
    ) -> Self {
        Self {
            attempts: 0,
            max_attempts,
            base_delay,
            max_delay,
            jitter_max_ms,
        }
    }

    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.attempts >= self.max_attempts {
            return None;
        }

        let exponential = self
            .base_delay
            .saturating_mul(2u32.saturating_pow(self.attempts));
        let jitter_ms = if self.jitter_max_ms > 0 {
            rand::thread_rng().gen_range(0..=self.jitter_max_ms)
        } else {
            0
        };
        let delayed = exponential.saturating_add(Duration::from_millis(jitter_ms));
        self.attempts = self.attempts.saturating_add(1);

        Some(delayed.min(self.max_delay))
    }

    pub fn reset(&mut self) {
        self.attempts = 0;
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    pub fn slow_retry_delay(&self) -> Duration {
        self.max_delay.saturating_mul(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_delay_progresses_and_caps() {
        let mut state = ReconnectState::new_with_jitter(
            4,
            Duration::from_millis(100),
            Duration::from_millis(350),
            0,
        );

        assert_eq!(state.next_delay(), Some(Duration::from_millis(100)));
        assert_eq!(state.next_delay(), Some(Duration::from_millis(200)));
        assert_eq!(state.next_delay(), Some(Duration::from_millis(350)));
        assert_eq!(state.next_delay(), Some(Duration::from_millis(350)));
        assert_eq!(state.next_delay(), None);
    }

    #[test]
    fn reset_clears_attempts() {
        let mut state = ReconnectState::new_with_jitter(
            3,
            Duration::from_millis(100),
            Duration::from_millis(500),
            0,
        );
        let _ = state.next_delay();
        let _ = state.next_delay();
        assert_eq!(state.attempts(), 2);
        state.reset();
        assert_eq!(state.attempts(), 0);
        assert_eq!(state.next_delay(), Some(Duration::from_millis(100)));
    }
}
