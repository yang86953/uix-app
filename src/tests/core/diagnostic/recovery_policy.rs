use crate::core::diagnostic::{
    CircuitBreaker, CircuitState, ExponentialBackoffRetryPolicy, RetryPolicy,
};
use std::time::Duration;

#[test]
fn half_open_circuit_admits_one_probe_and_reopens_after_probe_failure() {
    let circuit = CircuitBreaker::new(1, Duration::ZERO);
    circuit.record_failure();

    assert!(circuit.try_call());
    assert!(!circuit.try_call());
    assert_eq!(circuit.rejected_count(), 1);

    circuit.set_threshold(100);
    circuit.record_failure();
    assert!(circuit.try_call());

    circuit.record_success();
    assert_eq!(circuit.state(), CircuitState::Closed);
    assert!(circuit.try_call());
}

#[test]
fn circuit_reset_clears_state_and_all_metrics() {
    let circuit = CircuitBreaker::new(1, Duration::from_secs(60));
    circuit.record_success();
    circuit.record_failure();
    assert!(!circuit.try_call());

    circuit.reset();

    assert_eq!(circuit.state(), CircuitState::Closed);
    assert_eq!(circuit.failure_count(), 0);
    assert_eq!(circuit.success_count(), 0);
    assert_eq!(circuit.rejected_count(), 0);
}

#[test]
fn circuit_recovery_timeout_saturates_instead_of_wrapping() {
    let circuit = CircuitBreaker::default();

    circuit.set_recovery_timeout(Duration::MAX);

    assert_eq!(circuit.recovery_timeout(), Duration::from_millis(u64::MAX));
}

#[test]
fn circuit_failure_threshold_is_never_zero() {
    let circuit = CircuitBreaker::new(0, Duration::from_secs(60));

    assert_eq!(circuit.threshold(), 1);
    circuit.record_failure();
    assert_eq!(circuit.state(), CircuitState::Open);

    circuit.reset();
    circuit.set_threshold(0);
    assert_eq!(circuit.threshold(), 1);
    circuit.record_failure();
    assert_eq!(circuit.state(), CircuitState::Open);
}

#[test]
fn exponential_backoff_normalizes_invalid_float_configuration() {
    let non_finite = ExponentialBackoffRetryPolicy::new(3, 25, 1_000, f64::NAN, f64::INFINITY);
    assert_eq!(non_finite.delay(2), Duration::from_millis(25));
    assert_eq!(
        non_finite.describe(),
        "exponential_backoff(max=3, init=25ms, max_delay=1000ms, mult=1, jitter=0)"
    );

    let out_of_range = ExponentialBackoffRetryPolicy::new(3, 25, 1_000, 0.5, 2.0);
    assert_eq!(
        out_of_range.describe(),
        "exponential_backoff(max=3, init=25ms, max_delay=1000ms, mult=1, jitter=1)"
    );
}

#[test]
fn exponential_backoff_handles_extreme_attempts_without_iteration() {
    let policy = ExponentialBackoffRetryPolicy::new(usize::MAX, 10, 1_000, 2.0, 0.0);

    assert_eq!(policy.delay(0), Duration::from_millis(10));
    assert_eq!(policy.delay(1), Duration::from_millis(20));
    assert_eq!(policy.delay(usize::MAX), Duration::from_millis(1_000));
}
