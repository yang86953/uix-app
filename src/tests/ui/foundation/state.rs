use super::*;
use crate::draw::pipeline::InvalidationQueue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[test]
fn state_slot_id_is_unique_and_monotonic_for_new_state() {
    let first = State::new(1);
    let second = State::new(2);

    assert_ne!(first.slot_id(), second.slot_id());
    assert!(first.slot_id().get() < second.slot_id().get());
}

#[test]
fn state_clone_shares_slot_id() {
    let state = State::new("value");
    let cloned = state.clone();

    assert_eq!(state.slot_id(), cloned.slot_id());
}

#[test]
fn state_slot_id_is_stable_across_value_generation_changes() {
    let state = State::new(1);
    let slot_id = state.slot_id();
    let generation = state.generation();

    state.set(2);
    state.update(|value| *value += 1);

    assert_eq!(state.slot_id(), slot_id);
    assert!(state.generation() > generation);
}

#[test]
fn state_capture_fingerprint_is_stable_across_value_generation_changes() {
    let state = State::new(1);
    let fingerprint = state.capture_fingerprint();

    state.set(2);
    state.update(|value| *value += 1);

    assert_eq!(state.capture_fingerprint(), fingerprint);
}

#[test]
fn state_capture_fingerprint_includes_type_and_slot_id() {
    let int_state = State::new(1_i32);
    let other_int_state = State::new(2_i32);
    let string_state = State::new("1".to_string());

    assert_ne!(
        int_state.capture_fingerprint(),
        other_int_state.capture_fingerprint()
    );
    assert_ne!(
        int_state.capture_fingerprint(),
        string_state.capture_fingerprint()
    );
}

#[test]
fn state_set_fans_out_to_multiple_paint_sites() {
    let state = State::new(1);
    let first_queue = InvalidationQueue::shared();
    let second_queue = InvalidationQueue::shared();

    state.bind_paint_invalidation(
        10,
        first_queue.clone(),
        Some(Rect::new(0.0, 0.0, 10.0, 10.0)),
    );
    state.bind_paint_invalidation(
        20,
        second_queue.clone(),
        Some(Rect::new(5.0, 5.0, 10.0, 10.0)),
    );

    state.set(2);

    let first = first_queue.lock().unwrap_or_else(|e| e.into_inner());
    let second = second_queue.lock().unwrap_or_else(|e| e.into_inner());
    assert!(first.node_needs_paint(10));
    assert!(!first.node_needs_paint(20));
    assert!(second.node_needs_paint(20));
    assert!(!second.node_needs_paint(10));
}

#[test]
fn state_rebinding_same_paint_site_updates_in_place() {
    let state = State::new(1);
    let queue = InvalidationQueue::shared();

    state.bind_paint_invalidation(10, queue.clone(), None);
    state.bind_paint_invalidation(10, queue.clone(), Some(Rect::new(0.0, 0.0, 10.0, 10.0)));

    assert_eq!(
        state
            .paint_sites
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len(),
        1
    );

    state.set(2);

    let queue = queue.lock().unwrap_or_else(|e| e.into_inner());
    assert!(queue.node_needs_paint(10));
    assert!(!queue.needs_full_frame());
}

#[test]
fn computed_recompute_fans_out_to_multiple_paint_sites() {
    let source = State::new(1);
    let source_for_computed = source.clone();
    let computed = Computed::new(move || source_for_computed.get() * 2);
    let first_queue = InvalidationQueue::shared();
    let second_queue = InvalidationQueue::shared();

    computed.bind_paint_invalidation(
        10,
        first_queue.clone(),
        Some(Rect::new(0.0, 0.0, 10.0, 10.0)),
    );
    computed.bind_paint_invalidation(
        20,
        second_queue.clone(),
        Some(Rect::new(5.0, 5.0, 10.0, 10.0)),
    );

    source.set(2);
    assert_eq!(computed.get(), 4);

    let first = first_queue.lock().unwrap_or_else(|e| e.into_inner());
    let second = second_queue.lock().unwrap_or_else(|e| e.into_inner());
    assert!(first.node_needs_paint(10));
    assert!(second.node_needs_paint(20));
}

#[test]
fn state_set_fans_out_to_multiple_reconcile_sites() {
    let state = State::new(1);
    let first_hits = Arc::new(AtomicUsize::new(0));
    let second_hits = Arc::new(AtomicUsize::new(0));

    let first_for_callback = first_hits.clone();
    state.bind_reconcile_invalidation(
        1,
        Arc::new(move || {
            first_for_callback.fetch_add(1, Ordering::SeqCst);
        }),
    );
    let second_for_callback = second_hits.clone();
    state.bind_reconcile_invalidation(
        2,
        Arc::new(move || {
            second_for_callback.fetch_add(1, Ordering::SeqCst);
        }),
    );

    state.set(2);

    assert_eq!(first_hits.load(Ordering::SeqCst), 1);
    assert_eq!(second_hits.load(Ordering::SeqCst), 1);
}

#[test]
fn state_rebinding_same_reconcile_site_updates_in_place() {
    let state = State::new(1);
    let first_hits = Arc::new(AtomicUsize::new(0));
    let second_hits = Arc::new(AtomicUsize::new(0));

    let first_for_callback = first_hits.clone();
    state.bind_reconcile_invalidation(
        1,
        Arc::new(move || {
            first_for_callback.fetch_add(1, Ordering::SeqCst);
        }),
    );
    let second_for_callback = second_hits.clone();
    state.bind_reconcile_invalidation(
        1,
        Arc::new(move || {
            second_for_callback.fetch_add(1, Ordering::SeqCst);
        }),
    );

    assert_eq!(
        state
            .reconcile_sites
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len(),
        1
    );

    state.set(2);

    assert_eq!(first_hits.load(Ordering::SeqCst), 0);
    assert_eq!(second_hits.load(Ordering::SeqCst), 1);
}

#[test]
fn state_set_dirty_fn_replaces_reconcile_sites() {
    let state = State::new(1);
    let first_hits = Arc::new(AtomicUsize::new(0));
    let second_hits = Arc::new(AtomicUsize::new(0));

    let first_for_callback = first_hits.clone();
    state.bind_reconcile_invalidation(
        1,
        Arc::new(move || {
            first_for_callback.fetch_add(1, Ordering::SeqCst);
        }),
    );
    let second_for_callback = second_hits.clone();
    state.set_dirty_fn(move || {
        second_for_callback.fetch_add(1, Ordering::SeqCst);
    });

    state.set(2);

    assert_eq!(first_hits.load(Ordering::SeqCst), 0);
    assert_eq!(second_hits.load(Ordering::SeqCst), 1);
}
