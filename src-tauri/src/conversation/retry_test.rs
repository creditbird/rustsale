use super::retry::RetryPolicy;
use std::time::Duration;

#[test]
fn retry_policy_uses_five_bounded_backoff_steps() {
    let policy = RetryPolicy::default();
    assert_eq!(policy.delay(1, 0.5), Duration::from_secs(5));
    assert_eq!(policy.delay(2, 0.5), Duration::from_secs(15));
    assert_eq!(policy.delay(3, 0.5), Duration::from_secs(45));
    assert_eq!(policy.delay(4, 0.5), Duration::from_secs(120));
    assert_eq!(policy.delay(5, 0.5), Duration::from_secs(300));
    assert!(policy.should_stop(5));
}
