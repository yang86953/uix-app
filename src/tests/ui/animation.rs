use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::ui::animation::{Animation, Easing};

#[test]
fn documented_animation_surface_restarts_and_fires_finish_once() {
    let finished = Arc::new(AtomicUsize::new(0));
    let callback_count = Arc::clone(&finished);
    let mut animation = Animation::new(0.0_f32, 10.0, 1.0)
        .easing(Easing::Linear)
        .on_finish(move || {
            callback_count.fetch_add(1, Ordering::SeqCst);
        });

    assert_eq!(animation.value(), 0.0);
    assert_eq!(animation.update(0.25), 2.5);
    assert_eq!(finished.load(Ordering::SeqCst), 0);

    animation.restart();
    assert_eq!(animation.value(), 0.0);
    assert_eq!(animation.update(1.0), 10.0);
    assert_eq!(finished.load(Ordering::SeqCst), 1);

    let _ = animation.update(1.0);
    assert_eq!(finished.load(Ordering::SeqCst), 1);

    animation.pause();
    animation.reverse();
    assert!(animation.running);
    assert_eq!(animation.value(), 10.0);
}

#[test]
fn stopping_does_not_consume_a_pending_finish_callback() {
    let finished = Arc::new(AtomicUsize::new(0));
    let callback_count = Arc::clone(&finished);
    let mut animation = Animation::new(0.0_f64, 1.0, 0.5).on_finish(move || {
        callback_count.fetch_add(1, Ordering::SeqCst);
    });

    animation.stop();
    assert_eq!(animation.value(), 1.0);
    assert_eq!(finished.load(Ordering::SeqCst), 0);

    animation.restart();
    let _ = animation.update(0.5);
    assert_eq!(finished.load(Ordering::SeqCst), 1);
}

#[test]
fn cloned_animations_share_one_finish_callback() {
    let finished = Arc::new(AtomicUsize::new(0));
    let callback_count = Arc::clone(&finished);
    let mut first = Animation::new(0.0_f32, 1.0, 0.1).on_finish(move || {
        callback_count.fetch_add(1, Ordering::SeqCst);
    });
    let mut second = first.clone();

    let _ = first.update(0.1);
    let _ = second.update(0.1);
    assert_eq!(finished.load(Ordering::SeqCst), 1);
}

#[test]
fn non_positive_duration_finishes_on_the_first_update() {
    let finished = Arc::new(AtomicUsize::new(0));
    let callback_count = Arc::clone(&finished);
    let mut animation = Animation::new(2.0_f64, 4.0, -1.0).on_finish(move || {
        callback_count.fetch_add(1, Ordering::SeqCst);
    });

    assert_eq!(animation.duration, 0.0);
    assert!(animation.is_finished());
    assert_eq!(animation.update(0.0), 4.0);
    assert_eq!(finished.load(Ordering::SeqCst), 1);
}
