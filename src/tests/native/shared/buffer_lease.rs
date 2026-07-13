use crate::native::shared::buffer_lease::BufferLease;

fn acquire_next(leases: &[BufferLease; 2], active: usize) -> Option<usize> {
    [1 - active, active]
        .into_iter()
        .find(|&index| leases[index].try_acquire())
}

#[test]
fn compositor_backpressure_waits_until_a_buffer_is_released() {
    let leases = [BufferLease::new(), BufferLease::new()];

    let first = acquire_next(&leases, 0).unwrap();
    let second = acquire_next(&leases, first).unwrap();
    assert_ne!(first, second);
    for _ in 0..3 {
        assert_eq!(acquire_next(&leases, second), None);
    }

    leases[first].release();
    assert_eq!(acquire_next(&leases, second), Some(first));
}
