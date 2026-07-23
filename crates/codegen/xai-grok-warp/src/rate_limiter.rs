//! Rate limiter for WARP operations.
//!
//! Prevents excessive WARP reconnects by enforcing a minimum interval
//! between operations (default: 5 minutes).

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Thread-safe rate limiter based on wall-clock time.
///
/// Uses `Instant` for monotonic time (immune to system clock changes).
pub struct RateLimiter {
    last_run: Mutex<Option<Instant>>,
    interval: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter with the given minimum interval.
    pub fn new(interval: Duration) -> Self {
        Self {
            last_run: Mutex::new(None),
            interval,
        }
    }

    /// Check if enough time has elapsed since the last run.
    ///
    /// Returns `true` if the operation should proceed (either never run,
    /// or enough time has passed).
    pub fn should_run(&self) -> bool {
        let last = self.last_run.lock().unwrap();
        match *last {
            None => true,
            Some(instant) => instant.elapsed() >= self.interval,
        }
    }

    /// Record that the operation has been performed.
    ///
    /// Call this **after** a successful operation to start the cooldown.
    pub fn mark_run(&self) {
        *self.last_run.lock().unwrap() = Some(Instant::now());
    }

    /// Reset the rate limiter (clear last-run timestamp).
    pub fn reset(&self) {
        *self.last_run.lock().unwrap() = None;
    }

    /// Get the configured interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Get the time remaining until the next allowed run.
    ///
    /// Returns `Duration::ZERO` if the operation is allowed now.
    pub fn time_until_next(&self) -> Duration {
        let last = self.last_run.lock().unwrap();
        match *last {
            None => Duration::ZERO,
            Some(instant) => {
                let elapsed = instant.elapsed();
                if elapsed >= self.interval {
                    Duration::ZERO
                } else {
                    self.interval - elapsed
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_should_run_on_first_call() {
        let limiter = RateLimiter::new(Duration::from_secs(300));
        assert!(limiter.should_run());
    }

    #[test]
    fn rate_limiter_blocks_within_interval() {
        let limiter = RateLimiter::new(Duration::from_secs(300));
        limiter.mark_run();
        assert!(!limiter.should_run());
    }

    #[test]
    fn rate_limiter_allows_after_interval() {
        let limiter = RateLimiter::new(Duration::from_millis(1));
        limiter.mark_run();
        std::thread::sleep(Duration::from_millis(10));
        assert!(limiter.should_run());
    }

    #[test]
    fn rate_limiter_reset() {
        let limiter = RateLimiter::new(Duration::from_secs(300));
        limiter.mark_run();
        assert!(!limiter.should_run());
        limiter.reset();
        assert!(limiter.should_run());
    }

    #[test]
    fn rate_limiter_time_until_next() {
        let limiter = RateLimiter::new(Duration::from_secs(300));
        assert_eq!(limiter.time_until_next(), Duration::ZERO);

        limiter.mark_run();
        let remaining = limiter.time_until_next();
        assert!(
            remaining > Duration::ZERO,
            "Should have remaining time: {remaining:?}"
        );
        assert!(remaining <= Duration::from_secs(300));
    }

    #[test]
    fn rate_limiter_interval() {
        let limiter = RateLimiter::new(Duration::from_secs(60));
        assert_eq!(limiter.interval(), Duration::from_secs(60));
    }

    #[test]
    fn rate_limiter_thread_safe() {
        use std::sync::Arc;
        let limiter = Arc::new(RateLimiter::new(Duration::from_secs(300)));

        let limiter_clone = limiter.clone();
        std::thread::spawn(move || {
            limiter_clone.mark_run();
        })
        .join()
        .unwrap();

        assert!(!limiter.should_run());
    }
}
