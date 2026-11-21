//! Per-account retry pacing for the Messenger poll loop.
//!
//! Before this existed a failing page was retried on every poll tick (about every
//! 20-30 s in v2) and logged at `error` level each time: roughly 4,000 identical
//! lines a day for weeks. Now a failing account waits an exponentially growing delay
//! (capped), only the first failure of a streak (or a changed error) is logged as an
//! error, and the UI is told once when an account goes bad and once when it recovers.
//!
//! Auth failures never reach this module's retry path: the supervisor marks the
//! account `invalid`, which removes it from the connected list until the user
//! reconnects.

use std::time::{Duration, Instant};

/// First retry delay after a failure.
pub const BASE_DELAY: Duration = Duration::from_secs(30);
/// Upper bound of the delay between attempts.
pub const MAX_DELAY: Duration = Duration::from_secs(15 * 60);
/// Minimum wait after the Graph API says we are rate limited.
pub const RATE_LIMIT_FLOOR: Duration = Duration::from_secs(5 * 60);
/// While a streak keeps failing with the same error, log it again at this cadence.
const REMINDER_EVERY: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// Graph API throttled us (HTTP 429, codes 4/17/32/613).
    RateLimited,
    /// Anything else that may heal by itself (network, 5xx, locked DB, bad payload).
    Other,
}

/// What the supervisor should do about one failed attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureReport {
    pub retry_in: Duration,
    pub consecutive_failures: u32,
    /// Log at `error` level (first failure of a streak, a different error, or a reminder).
    pub log_as_error: bool,
    /// The account just turned unhealthy: tell the UI once.
    pub became_unhealthy: bool,
}

#[derive(Debug, Default)]
pub struct AccountSyncHealth {
    consecutive_failures: u32,
    retry_not_before: Option<Instant>,
    last_signature: Option<String>,
}

impl AccountSyncHealth {
    /// True when an attempt may be made at `now`.
    pub fn is_due(&self, now: Instant) -> bool {
        self.retry_not_before.is_none_or(|at| now >= at)
    }

    pub fn is_failing(&self) -> bool {
        self.consecutive_failures > 0
    }

    pub fn record_failure(
        &mut self,
        now: Instant,
        kind: FailureKind,
        signature: &str,
    ) -> FailureReport {
        let became_unhealthy = self.consecutive_failures == 0;
        let signature_changed = self.last_signature.as_deref() != Some(signature);
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);

        let retry_in = delay_for(self.consecutive_failures, kind);
        self.retry_not_before = now.checked_add(retry_in);
        self.last_signature = Some(signature.to_string());

        FailureReport {
            retry_in,
            consecutive_failures: self.consecutive_failures,
            log_as_error: became_unhealthy
                || signature_changed
                || self.consecutive_failures.is_multiple_of(REMINDER_EVERY),
            became_unhealthy,
        }
    }

    /// Resets the streak. Returns true when the account had been failing.
    pub fn record_success(&mut self) -> bool {
        let recovered = self.is_failing();
        *self = Self::default();
        recovered
    }
}

fn delay_for(consecutive_failures: u32, kind: FailureKind) -> Duration {
    let exponent = consecutive_failures.saturating_sub(1).min(16);
    let scaled = BASE_DELAY
        .checked_mul(1u32 << exponent)
        .unwrap_or(MAX_DELAY);
    let delay = match kind {
        FailureKind::RateLimited => scaled.max(RATE_LIMIT_FLOOR),
        FailureKind::Other => scaled,
    };
    delay.min(MAX_DELAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_account_is_always_due() {
        let health = AccountSyncHealth::default();
        assert!(health.is_due(Instant::now()));
        assert!(!health.is_failing());
    }

    #[test]
    fn failure_blocks_attempts_until_the_delay_has_passed() {
        let t0 = Instant::now();
        let mut health = AccountSyncHealth::default();
        let report = health.record_failure(t0, FailureKind::Other, "boom");

        assert_eq!(report.retry_in, BASE_DELAY);
        assert!(!health.is_due(t0));
        assert!(!health.is_due(t0 + BASE_DELAY - Duration::from_secs(1)));
        assert!(health.is_due(t0 + BASE_DELAY));
    }

    #[test]
    fn delay_doubles_and_is_capped() {
        let t0 = Instant::now();
        let mut health = AccountSyncHealth::default();
        let mut seen = Vec::new();
        for _ in 0..12 {
            seen.push(
                health
                    .record_failure(t0, FailureKind::Other, "boom")
                    .retry_in,
            );
        }
        assert_eq!(seen[0], Duration::from_secs(30));
        assert_eq!(seen[1], Duration::from_secs(60));
        assert_eq!(seen[2], Duration::from_secs(120));
        assert!(seen.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(*seen.last().unwrap_or(&Duration::ZERO), MAX_DELAY);
    }

    #[test]
    fn huge_failure_counts_do_not_overflow() {
        let t0 = Instant::now();
        let mut health = AccountSyncHealth {
            consecutive_failures: u32::MAX - 1,
            ..AccountSyncHealth::default()
        };
        let report = health.record_failure(t0, FailureKind::Other, "boom");
        assert_eq!(report.retry_in, MAX_DELAY);
    }

    #[test]
    fn rate_limit_waits_at_least_the_floor() {
        let mut health = AccountSyncHealth::default();
        let report = health.record_failure(Instant::now(), FailureKind::RateLimited, "429");
        assert_eq!(report.retry_in, RATE_LIMIT_FLOOR);
    }

    #[test]
    fn only_the_first_failure_of_a_streak_is_reported_and_logged_as_error() {
        let t0 = Instant::now();
        let mut health = AccountSyncHealth::default();

        let first = health.record_failure(t0, FailureKind::Other, "same");
        assert!(first.became_unhealthy && first.log_as_error);

        for _ in 0..5 {
            let again = health.record_failure(t0, FailureKind::Other, "same");
            assert!(!again.became_unhealthy);
            assert!(
                !again.log_as_error,
                "repeat of the same error must stay quiet"
            );
        }
    }

    #[test]
    fn a_different_error_is_logged_again_without_notifying_the_ui_twice() {
        let t0 = Instant::now();
        let mut health = AccountSyncHealth::default();
        health.record_failure(t0, FailureKind::Other, "first");
        let changed = health.record_failure(t0, FailureKind::Other, "second");
        assert!(changed.log_as_error);
        assert!(!changed.became_unhealthy);
    }

    #[test]
    fn long_streak_gets_a_periodic_reminder() {
        let t0 = Instant::now();
        let mut health = AccountSyncHealth::default();
        let logged: Vec<u32> = (0..40)
            .map(|_| health.record_failure(t0, FailureKind::Other, "same"))
            .filter(|r| r.log_as_error)
            .map(|r| r.consecutive_failures)
            .collect();
        assert_eq!(logged, vec![1, 20, 40]);
    }

    #[test]
    fn success_resets_and_reports_recovery_once() {
        let t0 = Instant::now();
        let mut health = AccountSyncHealth::default();
        assert!(
            !health.record_success(),
            "healthy account has nothing to recover from"
        );

        health.record_failure(t0, FailureKind::Other, "boom");
        assert!(health.record_success());
        assert!(!health.record_success());
        assert!(health.is_due(t0));

        let next = health.record_failure(t0, FailureKind::Other, "boom");
        assert!(next.became_unhealthy, "a new streak notifies the UI again");
        assert_eq!(next.retry_in, BASE_DELAY);
    }
}
