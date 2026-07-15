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
