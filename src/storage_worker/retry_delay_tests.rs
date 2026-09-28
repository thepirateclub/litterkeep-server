//! Проверки интервалов повторного пакетного DSN sweep.

use super::*;

#[test]
fn first_failure_waits_initial_interval() {
    let now = Instant::now();
    let mut retry = RetryDelay::new();
    assert!(retry.is_ready(now));
    retry.failed(now);
    assert!(!retry.is_ready(now + Duration::from_millis(99)));
    assert!(retry.is_ready(now + Duration::from_millis(100)));
}

#[test]
fn repeated_failures_double_delay_and_cap_at_thirty_seconds() {
    let mut retry = RetryDelay::new();
    let mut now = Instant::now();
    let delays = [
        100, 200, 400, 800, 1_600, 3_200, 6_400, 12_800, 25_600, 30_000,
    ];
    for millis in delays {
        retry.failed(now);
        now += Duration::from_millis(millis);
        assert!(retry.is_ready(now));
        assert!(!retry.is_ready(now - Duration::from_nanos(1)));
    }
    retry.failed(now);
    assert!(!retry.is_ready(now + Duration::from_secs(29)));
    assert!(retry.is_ready(now + Duration::from_secs(30)));
}

#[test]
fn successful_sweep_resets_retry_interval() {
    let now = Instant::now();
    let mut retry = RetryDelay::new();
    retry.failed(now);
    retry.failed(now + Duration::from_millis(100));
    retry.succeeded();
    assert!(retry.is_ready(now + Duration::from_millis(100)));
    retry.failed(now + Duration::from_millis(100));
    assert!(!retry.is_ready(now + Duration::from_millis(199)));
    assert!(retry.is_ready(now + Duration::from_millis(200)));
}
