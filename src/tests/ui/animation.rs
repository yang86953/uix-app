use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::app::active_work_registry::{ActiveWorkKind, ActiveWorkRegistry};
use crate::app::window_driver::sync_animation_registrations;
use crate::core::Point;
use crate::ui::animation::{Animated, Animation, AnimationConfig, Easing, TransitionPlayer};
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::{label, ViewAdapter, ViewNode};
use crate::ui::Placement;

fn animated_root(animated: &Animated<f32>) -> ViewNode {
    ViewAdapter::capture_root(|| label("fade").opacity(animated.value()))
}

#[test]
fn animated_value_registers_without_adding_layout_nodes() {
    let animated = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));
    let root = tree.root_id().expect("animated root");

    assert!(tree
        .get(root)
        .expect("animated root node")
        .children()
        .is_empty());

    let updates = tree.update_animations(0.0);
    assert_eq!(updates.len(), 1);
    assert!(updates[0].1);
    assert_eq!(animated.value(), 0.0);
    assert!(!tree.take_reconcile_requested());

    let updates = tree.update_animations(0.5);
    assert_eq!(updates.len(), 1);
    assert!(updates[0].1);
    assert!((animated.value() - 0.5).abs() < 1e-6);
    assert!(tree.take_reconcile_requested());

    let updates = tree.update_animations(0.5);
    assert_eq!(updates.len(), 1);
    assert!(!updates[0].1);
    assert!((animated.value() - 1.0).abs() < 1e-6);
    assert!(tree.update_animations(0.1).is_empty());
}

#[test]
fn animated_controls_stop_and_restart_frame_work() {
    let animated = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));

    let _ = tree.update_animations(0.25);
    animated.pause();
    assert!(tree.update_animations(0.25).is_empty());
    assert!((animated.value() - 0.25).abs() < 1e-6);
    assert!(!animated.is_finished());

    animated.resume();
    let updates = tree.update_animations(0.25);
    assert_eq!(updates.len(), 1);
    assert!(updates[0].1);
    assert!((animated.value() - 0.5).abs() < 1e-6);

    animated.stop();
    assert!((animated.value() - 1.0).abs() < 1e-6);
    assert!(tree.update_animations(0.25).is_empty());

    animated.restart();
    assert!((animated.value() - 0.0).abs() < 1e-6);
    let _ = tree.update_animations(0.5);
    assert!((animated.value() - 0.5).abs() < 1e-6);

    animated.reverse();
    assert!((animated.value() - 1.0).abs() < 1e-6);
    let _ = tree.update_animations(0.25);
    assert!((animated.value() - 0.75).abs() < 1e-6);
}

#[test]
fn animated_value_has_one_driving_window() {
    let animated = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    let mut owner = ViewAdapter::build_nodes(animated_root(&animated));
    let mut observer = ViewAdapter::build_nodes(animated_root(&animated));

    assert_eq!(owner.update_animations(0.5).len(), 1);
    assert!(observer.update_animations(0.5).is_empty());
    assert!((animated.value() - 0.5).abs() < 1e-6);

    drop(owner);
    animated.restart();
    ViewAdapter::reconcile_nodes(&mut observer, animated_root(&animated));
    assert_eq!(observer.update_animations(0.5).len(), 1);
    assert!((animated.value() - 0.5).abs() < 1e-6);
}

#[test]
fn zero_duration_animated_value_commits_without_frame_work() {
    let animated = Animated::new(2.0_f64).to(4.0, -1.0, Easing::linear);
    assert_eq!(animated.value(), 4.0);
    assert!(animated.is_finished());
    assert_eq!(animated.progress(), 1.0);
}

#[test]
fn delayed_animated_waits_on_one_deadline_before_open_frame_work() {
    let before_creation = Instant::now();
    let animated = Animated::new(0.0_f32).to_after(5.0, 1.0, 1.0, Easing::linear);
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));

    let updates = tree.update_animations_at(before_creation, 1.0);
    assert_eq!(updates.len(), 1);
    assert!(!updates[0].1);
    assert_eq!(animated.value(), 0.0);
    assert_eq!(animated.progress(), 0.0);
    assert!(!animated.is_finished());

    let work_id = updates[0].0;
    let mut active_work = ActiveWorkRegistry::new();
    sync_animation_registrations(&mut active_work, &tree, &updates);
    let deadline = active_work
        .next_deadline()
        .expect("delayed Animated deadline");
    assert_eq!(active_work.next_deadline(), Some(deadline));

    let before_deadline = deadline - Duration::from_nanos(1);
    let updates = tree.update_animations_at(before_deadline, 1.0);
    sync_animation_registrations(&mut active_work, &tree, &updates);
    assert_eq!(animated.value(), 0.0);
    assert_eq!(active_work.next_deadline(), Some(deadline));

    assert_eq!(
        active_work.drain_due(deadline),
        vec![ActiveWorkKind::Animation(work_id)]
    );
    let updates = tree.update_animations_at(deadline, 0.0);
    sync_animation_registrations(&mut active_work, &tree, &updates);
    assert_eq!(updates, vec![(work_id, true)]);
    assert_eq!(active_work.next_deadline(), None);
    assert_eq!(
        active_work.animation_ids().collect::<Vec<_>>(),
        vec![work_id]
    );

    let _ = tree.update_animations_at(deadline + Duration::from_millis(500), 0.5);
    assert!((animated.value() - 0.5).abs() < 1e-6);
}

#[test]
fn delayed_zero_duration_commits_only_when_its_deadline_is_due() {
    let animated = Animated::new(2.0_f32).to_after(1.0, 4.0, 0.0, Easing::linear);
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));
    let pending = tree.update_animations(0.0);
    let deadline = tree.animated_source_registrations()[0]
        .1
        .expect("delayed zero-duration deadline");

    assert_eq!(animated.value(), 2.0);
    assert!(!animated.is_finished());

    let updates = tree.update_animations_at(deadline, 0.0);
    assert_eq!(updates, vec![(pending[0].0, false)]);
    assert_eq!(animated.value(), 4.0);
    assert!(animated.is_finished());
    assert_eq!(animated.progress(), 1.0);
}

#[test]
fn delayed_pause_and_root_removal_cancel_managed_deadline_work() {
    let animated = Animated::new(0.0_f32).to_after(5.0, 1.0, 1.0, Easing::linear);
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));
    let updates = tree.update_animations(0.0);
    let mut active_work = ActiveWorkRegistry::new();
    sync_animation_registrations(&mut active_work, &tree, &updates);
    assert!(active_work.next_deadline().is_some());

    animated.pause();
    assert!(tree.update_animations(0.0).is_empty());
    sync_animation_registrations(&mut active_work, &tree, &[]);
    assert!(active_work.is_empty());

    animated.resume();
    sync_animation_registrations(&mut active_work, &tree, &[]);
    assert!(active_work.next_deadline().is_some());

    ViewAdapter::reconcile_nodes(&mut tree, ViewAdapter::capture_root(|| label("resting")));
    sync_animation_registrations(&mut active_work, &tree, &[]);
    assert!(active_work.is_empty());
}

#[test]
fn counted_animated_loop_consumes_large_deltas_and_stops_at_target() {
    let animated = Animated::new(0.0_f32)
        .to(1.0, 1.0, Easing::linear)
        .loop_count(3);
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));

    let updates = tree.update_animations(2.5);
    assert_eq!(updates.len(), 1);
    assert!(updates[0].1);
    assert!((animated.value() - 0.5).abs() < 1e-6);
    assert!((animated.progress() - 0.5).abs() < 1e-6);
    assert!(!animated.is_finished());

    let updates = tree.update_animations(0.5);
    assert_eq!(updates.len(), 1);
    assert!(!updates[0].1);
    assert_eq!(animated.value(), 1.0);
    assert!(animated.is_finished());
    assert!(tree.update_animations(1.0).is_empty());
}

#[test]
fn forever_and_alternate_animated_loops_keep_constant_frame_work() {
    let repeating = Animated::new(0.0_f32)
        .to(1.0, 1.0, Easing::linear)
        .loop_forever();
    let alternating = Animated::new(0.0_f32)
        .to(1.0, 1.0, Easing::linear)
        .loop_alternate();
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        crate::ui::view::column((
            label("repeat").opacity(repeating.value()),
            label("alternate").opacity(alternating.value()),
        ))
    }));

    let updates = tree.update_animations(2.25);
    assert_eq!(updates.len(), 2);
    assert!(updates.iter().all(|(_, active)| *active));
    assert!((repeating.value() - 0.25).abs() < 1e-6);
    assert!((alternating.value() - 0.25).abs() < 1e-6);

    let _ = tree.update_animations(1.0);
    assert!((repeating.value() - 0.25).abs() < 1e-6);
    assert!((alternating.value() - 0.75).abs() < 1e-6);
    assert!(!repeating.is_finished());
    assert!(!alternating.is_finished());

    repeating.stop();
    assert!(repeating.is_finished());
    let updates = tree.update_animations(0.25);
    assert_eq!(updates.len(), 1);
    assert!(updates[0].1);
}

#[test]
fn zero_count_animated_loop_commits_without_frame_work() {
    let animated = Animated::new(0.0_f32)
        .to(1.0, 1.0, Easing::linear)
        .loop_count(0);
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));

    assert_eq!(animated.value(), 1.0);
    assert!(animated.is_finished());
    assert!(tree.update_animations(1.0).is_empty());
}

#[test]
fn counted_loop_restart_and_reverse_reset_the_whole_sequence() {
    let animated = Animated::new(0.0_f32)
        .to(1.0, 1.0, Easing::linear)
        .loop_count(2);
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));

    let _ = tree.update_animations(1.25);
    assert!((animated.value() - 0.25).abs() < 1e-6);

    animated.restart();
    assert_eq!(animated.value(), 0.0);
    let updates = tree.update_animations(2.0);
    assert_eq!(updates.len(), 1);
    assert!(!updates[0].1);
    assert_eq!(animated.value(), 1.0);

    animated.reverse();
    assert_eq!(animated.value(), 1.0);
    let updates = tree.update_animations(2.0);
    assert_eq!(updates.len(), 1);
    assert!(!updates[0].1);
    assert_eq!(animated.value(), 0.0);
}

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

#[test]
fn animation_config_constructors_normalize_duration_and_phase() {
    let enter = AnimationConfig::zoom_in(-1.0);
    let leave = AnimationConfig::fade_out(0.15);

    assert!(enter.is_enter());
    assert!(!enter.is_exit());
    assert_eq!(enter.duration(), 0.0);
    assert!(leave.is_exit());
    assert_eq!(leave.duration(), 0.15);
}

#[test]
fn slide_config_uses_placement_as_the_visual_origin() {
    let cases = [
        (Placement::Top, Point::new(0.0, -24.0)),
        (Placement::Bottom, Point::new(0.0, 24.0)),
        (Placement::Left, Point::new(-24.0, 0.0)),
        (Placement::Right, Point::new(24.0, 0.0)),
    ];

    for (placement, expected_offset) in cases {
        let player = TransitionPlayer::new(AnimationConfig::slide_in(placement, 0.2));

        assert!((player.offset.x - expected_offset.x).abs() < 1e-4);
        assert!((player.offset.y - expected_offset.y).abs() < 1e-4);
        assert_eq!(player.opacity_progress, 0.0);
        assert_eq!(player.scale, 1.0);
    }
}

#[test]
fn fade_config_does_not_apply_zoom_scale() {
    let enter = TransitionPlayer::new(AnimationConfig::fade_in(0.2));
    let leave = TransitionPlayer::new(AnimationConfig::fade_out(0.2));

    assert_eq!(enter.scale, 1.0);
    assert_eq!(leave.scale, 1.0);
}
