use crate::core::diagnostic::{CircuitBreaker, CircuitState};
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
