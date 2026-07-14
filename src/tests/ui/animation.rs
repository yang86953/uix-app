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

#[test]
fn easing_aliases_match_their_documented_curves() {
    let t = 0.25;
    assert_eq!(Easing::linear.sample(t), Easing::Linear.sample(t));
    assert_eq!(Easing::ease_in.sample(t), Easing::QuadIn.sample(t));
    assert_eq!(Easing::ease_out.sample(t), Easing::QuadOut.sample(t));
    assert_eq!(Easing::ease_in_out.sample(t), Easing::QuadInOut.sample(t));
    assert_eq!(Easing::cubic_in.sample(t), Easing::CubicIn.sample(t));
    assert_eq!(Easing::cubic_out.sample(t), Easing::CubicOut.sample(t));
    assert_eq!(Easing::cubic_in_out.sample(t), Easing::CubicInOut.sample(t));
    assert_eq!(
        Easing::cubic_bezier(0.25, 0.1, 0.25, 1.0),
        Easing::antd_default()
    );
}

#[test]
fn bounce_and_elastic_preserve_endpoints_and_expose_rebound_motion() {
    for easing in [Easing::bounce, Easing::elastic] {
        assert_eq!(easing.sample(0.0), 0.0);
        assert_eq!(easing.sample(1.0), 1.0);
    }

    for step in 0..=1000 {
        let sample = Easing::bounce.sample(step as f64 / 1000.0);
        assert!((0.0..=1.0).contains(&sample));
    }

    assert!(Easing::bounce.sample(0.36) > Easing::bounce.sample(0.55));
    assert!(Easing::elastic.sample(0.1) > 1.0);
}
