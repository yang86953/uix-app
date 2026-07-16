use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::app::active_work_registry::{ActiveWorkKind, ActiveWorkRegistry};
use crate::app::window_driver::sync_animation_registrations;
use crate::core::Point;
use crate::draw::Color;
use crate::ui::animation::{
    Animated, Animation, AnimationConfig, AnimationGroup, AnimationGroupError, Easing, Keyframe,
    KeyframeAnimation, KeyframeError, Spring, SpringAnimation, TransitionPlayer,
};
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::{column, label, ViewAdapter, ViewNode};
use crate::ui::Placement;

crate::keyframe! {
    #[derive(Debug, PartialEq)]
    struct MotionFrame {
        opacity: f32,
        offset: Point,
    }
}

fn animated_root(animated: &Animated<f32>) -> ViewNode {
    ViewAdapter::capture_root(|| label("fade").opacity(animated.value()))
}

#[test]
fn newly_reconciled_mount_transition_registers_without_an_extra_event() {
    let mut tree = ViewAdapter::build(crate::ui::view::column(Vec::<ViewNode>::new()));
    ViewAdapter::reconcile(
        &mut tree,
        crate::ui::view::column((label("new").enter_animation(AnimationConfig::fade_in(1.0)),)),
    );
    let root = tree.root_id().expect("root");
    let child = tree.get(root).unwrap().children()[0];
    let mut active_work = ActiveWorkRegistry::new();

    sync_animation_registrations(&mut active_work, &tree, &[]);
    assert_eq!(active_work.animation_ids().collect::<Vec<_>>(), vec![child]);

    tree.layout();
    let updates = tree.update_animation_nodes([child], 1.0);
    sync_animation_registrations(&mut active_work, &tree, &updates);
    assert_eq!(updates, vec![(child, false)]);
    assert!(active_work.is_empty());
}

#[test]
fn staggered_mount_transitions_wait_on_deadlines_without_open_frame_work() {
    let mut tree = ViewAdapter::build(
        column((label("first"), label("second"), label("third")))
            .stagger_enter(0.5, AnimationConfig::fade_in(1.0)),
    );
    tree.layout();
    let root = tree.root_id().expect("root");
    let children = tree.get(root).unwrap().children().to_vec();
    let registrations = tree.view_transition_registrations();
    assert_eq!(registrations.len(), 3);
    assert_eq!(registrations[0], (children[0], None));
    let second_deadline = registrations[1].1.expect("second deadline");
    let third_deadline = registrations[2].1.expect("third deadline");
    assert_eq!(
        third_deadline.duration_since(second_deadline),
        Duration::from_millis(500)
    );

    let mut active_work = ActiveWorkRegistry::new();
    sync_animation_registrations(&mut active_work, &tree, &[]);
    assert_eq!(
        active_work.animation_ids().collect::<Vec<_>>(),
        vec![children[0]]
    );
    assert_eq!(active_work.next_deadline(), Some(second_deadline));

    let updates = tree.update_animation_nodes_at(
        [children[1]],
        second_deadline - Duration::from_nanos(1),
        1.0,
    );
    sync_animation_registrations(&mut active_work, &tree, &updates);
    assert_eq!(updates, vec![(children[1], false)]);
    assert_eq!(
        tree.get(children[1]).unwrap().view_transition_opacity(),
        0.0
    );
    assert_eq!(active_work.next_deadline(), Some(second_deadline));

    assert_eq!(
        active_work.drain_due(second_deadline),
        vec![ActiveWorkKind::Animation(children[1])]
    );
    let updates = tree.update_animation_nodes_at([children[1]], second_deadline, 0.0);
    sync_animation_registrations(&mut active_work, &tree, &updates);
    assert_eq!(updates, vec![(children[1], true)]);
    assert_eq!(active_work.next_deadline(), Some(third_deadline));
    assert_eq!(
        active_work.animation_ids().collect::<Vec<_>>(),
        vec![children[0], children[1]]
    );
}

#[test]
fn staggered_mount_consumes_time_past_a_late_deadline() {
    let mut tree = ViewAdapter::build(
        column((label("first"), label("late"))).stagger_enter(1.0, AnimationConfig::fade_in(1.0)),
    );
    tree.layout();
    let root = tree.root_id().expect("root");
    let late = tree.get(root).unwrap().children()[1];
    let deadline = tree
        .view_transition_registrations()
        .into_iter()
        .find_map(|(id, deadline)| (id == late).then_some(deadline).flatten())
        .expect("late deadline");

    let updates =
        tree.update_animation_nodes_at([late], deadline + Duration::from_millis(500), 0.0);

    assert_eq!(updates, vec![(late, true)]);
    let opacity = tree.get(late).unwrap().view_transition_opacity();
    assert!(opacity > 0.0 && opacity < 1.0);
    assert_eq!(
        tree.view_transition_registrations()
            .into_iter()
            .find(|(id, _)| *id == late),
        Some((late, None))
    );
}

#[test]
fn dynamic_stagger_only_schedules_new_mounts_and_keeps_their_deadline() {
    let list = |items: &[&str]| {
        column(
            items
                .iter()
                .map(|item| label(*item).key(*item))
                .collect::<Vec<_>>(),
        )
        .stagger_enter(0.25, AnimationConfig::slide_in(Placement::Bottom, 1.0))
    };
    let mut tree = ViewAdapter::build(list(&["stable"]));
    tree.layout();
    let root = tree.root_id().expect("root");
    let stable = tree.get(root).unwrap().children()[0];
    assert_eq!(
        tree.update_animation_nodes([stable], 1.0),
        vec![(stable, false)]
    );

    ViewAdapter::reconcile(&mut tree, list(&["stable", "new-a", "new-b"]));
    tree.layout();

    let children = tree.get(root).unwrap().children().to_vec();
    assert_eq!(children[0], stable);
    let registrations = tree.view_transition_registrations();
    assert_eq!(registrations.len(), 2);
    assert_eq!(registrations[0], (children[1], None));
    let deadline = registrations[1].1.expect("second new mount deadline");

    ViewAdapter::reconcile(&mut tree, list(&["stable", "new-a", "new-b"]));

    assert_eq!(tree.get(root).unwrap().children(), children.as_slice());
    assert_eq!(
        tree.view_transition_registrations()
            .into_iter()
            .find(|(id, _)| *id == children[2]),
        Some((children[2], Some(deadline)))
    );
}

#[test]
fn staggered_zero_duration_mount_commits_only_at_its_deadline() {
    let mut tree = ViewAdapter::build(
        column((label("first"), label("delayed")))
            .stagger_enter(0.5, AnimationConfig::fade_in(0.0)),
    );
    tree.layout();
    let root = tree.root_id().expect("root");
    let delayed = tree.get(root).unwrap().children()[1];
    let deadline = tree
        .view_transition_registrations()
        .into_iter()
        .find_map(|(id, deadline)| (id == delayed).then_some(deadline).flatten())
        .expect("zero-duration deadline");
    assert_eq!(tree.get(delayed).unwrap().view_transition_opacity(), 0.0);

    assert_eq!(
        tree.update_animation_nodes_at([delayed], deadline, 0.0),
        vec![(delayed, false)]
    );
    assert_eq!(tree.get(delayed).unwrap().view_transition_opacity(), 1.0);
    assert!(tree.view_transition_registrations().is_empty());
}

#[test]
#[should_panic(expected = "stagger_enter requires fade_in, slide_in, or zoom_in")]
fn stagger_enter_rejects_exit_presets() {
    let _ = column((label("invalid"),)).stagger_enter(0.1, AnimationConfig::fade_out(0.2));
}

#[test]
fn non_finite_stagger_interval_starts_all_children_immediately() {
    let tree = ViewAdapter::build(
        column((label("first"), label("second")))
            .stagger_enter(f64::NAN, AnimationConfig::fade_in(1.0)),
    );

    assert!(tree
        .view_transition_registrations()
        .into_iter()
        .all(|(_, deadline)| deadline.is_none()));
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
fn animated_style_bindings_capture_sources_without_wrapper_nodes() {
    let opacity = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    let background = Animated::new(Color::black()).to(Color::white(), 1.0, Easing::linear);
    let foreground = Animated::new(Color::red()).to(Color::blue(), 1.0, Easing::linear);
    let width = Animated::new(40.0_f32).to(80.0, 1.0, Easing::linear);
    let height = Animated::new(20.0_f32).to(60.0, 1.0, Easing::linear);
    let radius = Animated::new(0.0_f32).to(8.0, 1.0, Easing::linear);
    let offset =
        Animated::new(Point::new(0.0, 0.0)).to(Point::new(20.0, 10.0), 1.0, Easing::linear);
    let scale = Animated::new(1.0_f32).to(1.5, 1.0, Easing::linear);

    let build = || {
        ViewAdapter::capture_root(|| {
            label("bound")
                .opacity_animated(&opacity)
                .bg_animated(&background)
                .color_animated(&foreground)
                .width_animated(&width)
                .height_animated(&height)
                .radius_animated(&radius)
                .offset_animated(&offset)
                .scale_animated(&scale)
        })
    };
    let root = build();
    assert_eq!(root.animated_sources.len(), 8);
    assert_eq!(root.style.opacity, 0.0);
    assert_eq!(root.style.background, Some(background.value().into()));
    assert_eq!(root.style.color, foreground.value().into());
    assert_eq!(root.style.width, Some(40.0));
    assert_eq!(root.style.height, Some(20.0));
    assert_eq!(root.style.border_radius, 0.0);
    assert_eq!(root.visual_transform.offset, Point::new(0.0, 0.0));
    assert_eq!(root.visual_transform.scale, 1.0);

    let mut tree = ViewAdapter::build_nodes(root);
    let root_id = tree.root_id().expect("animated style root");
    assert_eq!(tree.traverse().len(), 1);
    assert_eq!(tree.animated_source_registrations().len(), 8);
    assert_eq!(tree.update_animations(0.5).len(), 8);

    let next = build();
    assert_eq!(next.animated_sources.len(), 8);
    assert!((next.style.opacity - 0.5).abs() < 1e-6);
    assert_eq!(next.style.background, Some(background.value().into()));
    assert_eq!(next.style.color, foreground.value().into());
    assert_eq!(next.style.width, Some(60.0));
    assert_eq!(next.style.height, Some(40.0));
    assert_eq!(next.style.border_radius, 4.0);
    assert_eq!(next.visual_transform.offset, Point::new(10.0, 5.0));
    assert!((next.visual_transform.scale - 1.25).abs() < 1e-6);
    ViewAdapter::reconcile_nodes(&mut tree, next);
    assert_eq!(tree.root_id(), Some(root_id));
    assert_eq!(tree.traverse().len(), 1);
}

#[test]
fn keyframe_macro_properties_share_one_timeline_and_registration() {
    let from = MotionFrame::new(0.0, Point::new(0.0, 0.0));
    let to = MotionFrame::new(1.0, Point::new(3.0, 4.0));
    assert_eq!(
        <MotionFrame as crate::ui::Animatable>::lerp(from, to, 0.5),
        MotionFrame::new(0.5, Point::new(1.5, 2.0))
    );
    assert_eq!(<MotionFrame as crate::ui::Animatable>::delta(from, to), 5.0);

    let animated = Animated::new(from)
        .to_keyframes([Keyframe::new(0.0, from), Keyframe::new(1.0, to)], 1.0)
        .expect("aggregate keyframe sequence");
    let root_value = animated.clone();
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(move || {
        let current = root_value.value();
        label(format!("offset={}", current.offset.y)).opacity(current.opacity)
    }));

    assert_eq!(tree.animated_source_registrations().len(), 1);
    let updates = tree.update_animations(0.5);
    assert_eq!(updates.len(), 1);
    assert!(updates[0].1);
    assert_eq!(
        animated.value(),
        MotionFrame::new(0.5, Point::new(1.5, 2.0))
    );

    let error = animated
        .animate_keyframes([], 1.0)
        .expect_err("empty aggregate sequence should fail");
    assert_eq!(error, KeyframeError::Empty);
    assert_eq!(tree.update_animations(0.5).len(), 1);
    assert_eq!(animated.value(), to);
    assert!(animated.is_finished());
}

#[test]
fn parallel_animation_group_reuses_children_and_pauses_them_together() {
    let opacity = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    let offset = Animated::new(Point::new(0.0, 0.0)).to(Point::new(0.0, 20.0), 2.0, Easing::linear);
    let group = AnimationGroup::parallel([opacity.group_item(), offset.group_item()])
        .expect("finite parallel group");
    assert_eq!(group.duration(), 2.0);

    let root_group = group.clone();
    let root_opacity = opacity.clone();
    let root_offset = offset.clone();
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(move || {
        let _ = root_group.progress();
        label(format!("offset={}", root_offset.value().y)).opacity(root_opacity.value())
    }));
    assert_eq!(tree.animated_source_registrations().len(), 2);

    let updates = tree.update_animations(0.5);
    assert_eq!(updates.len(), 2);
    assert!((opacity.value() - 0.5).abs() < 1e-6);
    assert!((offset.value().y - 5.0).abs() < 1e-6);
    assert!((group.progress() - 0.25).abs() < 1e-6);

    group.pause();
    assert!(tree.update_animations(1.0).is_empty());
    assert!((opacity.value() - 0.5).abs() < 1e-6);
    assert!((offset.value().y - 5.0).abs() < 1e-6);

    group.resume();
    assert_eq!(tree.update_animations(0.5).len(), 2);
    assert_eq!(opacity.value(), 1.0);
    assert!((offset.value().y - 10.0).abs() < 1e-6);
    assert!((group.progress() - 0.5).abs() < 1e-6);
}

#[test]
fn sequential_animation_group_consumes_large_delta_across_delay() {
    let first = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    let second = Animated::new(10.0_f32).to(20.0, 1.0, Easing::linear);
    let before_group = Instant::now();
    let group = AnimationGroup::sequential([
        first.group_item(),
        AnimationGroup::delay(0.5),
        second.group_item(),
    ])
    .expect("finite sequential group");
    let after_group = Instant::now();
    assert_eq!(group.duration(), 2.5);
    assert!(group.progress().abs() < 1e-6);

    let root_group = group.clone();
    let root_first = first.clone();
    let root_second = second.clone();
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(move || {
        let _ = root_group.progress();
        label(format!("{}:{}", root_first.value(), root_second.value()))
    }));
    assert_eq!(tree.animated_source_registrations().len(), 3);

    let updates = tree.update_animations_at(after_group + Duration::from_secs(2), 2.0);
    assert_eq!(updates.len(), 3);
    assert_eq!(first.value(), 1.0);
    assert!(second.value() >= 14.9 && second.value() <= 15.1);
    assert!(group.progress() >= 0.79 && group.progress() <= 0.81);

    let _ = tree.update_animations_at(before_group + Duration::from_secs(10), 8.0);
    assert_eq!(second.value(), 20.0);
    assert_eq!(group.progress(), 1.0);
    assert!(group.is_finished());

    group.restart();
    assert_eq!(first.value(), 0.0);
    assert_eq!(second.value(), 10.0);
    assert!(!group.is_finished());
    group.stop();
    assert_eq!(first.value(), 1.0);
    assert_eq!(second.value(), 20.0);
    assert!(group.is_finished());
}

#[test]
fn staggered_animation_group_offsets_existing_source_deadlines() {
    let first = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    let second = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    let third = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    let group = AnimationGroup::stagger(
        [first.group_item(), second.group_item(), third.group_item()],
        0.25,
    )
    .expect("finite sources should form a staggered group");
    assert_eq!(group.duration(), 1.5);
    assert!(group.progress().abs() < 1e-6);

    let root_group = group.clone();
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(move || {
        let _ = root_group.progress();
        label("stagger")
    }));
    let registrations = tree.animated_source_registrations();
    assert_eq!(registrations.len(), 3);
    assert_eq!(
        registrations
            .iter()
            .filter(|(_, due)| due.is_none())
            .count(),
        1
    );
    assert_eq!(
        registrations
            .iter()
            .filter(|(_, due)| due.is_some())
            .count(),
        2
    );
    let last_deadline = registrations
        .iter()
        .filter_map(|(_, due)| *due)
        .max()
        .expect("staggered children should have deadlines");

    let updates = tree.update_animations_at(last_deadline, 0.5);
    assert_eq!(updates.len(), 3);
    assert!(first.value() >= 0.49 && first.value() <= 0.51);
    assert!(second.value() >= 0.24 && second.value() <= 0.26);
    assert!(third.value().abs() < 1e-6);
    assert!(group.progress() >= 0.32 && group.progress() <= 0.34);

    let _ = tree.update_animations_at(last_deadline + Duration::from_secs(2), 2.0);
    assert!(group.is_finished());
    assert_eq!(group.progress(), 1.0);
}

#[test]
fn counted_animation_group_progress_spans_all_plays() {
    let repeated = Animated::new(0.0_f32)
        .to(1.0, 1.0, Easing::linear)
        .loop_count(3);
    let group = AnimationGroup::parallel([repeated.group_item()])
        .expect("counted playback has a finite group duration");
    assert_eq!(group.duration(), 3.0);

    let root_group = group.clone();
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(move || {
        let _ = root_group.progress();
        label("counted")
    }));
    assert_eq!(tree.update_animations(1.5).len(), 1);
    assert!((repeated.value() - 0.5).abs() < 1e-6);
    assert!((group.progress() - 0.5).abs() < 1e-6);
}

#[test]
fn animated_finish_callback_observes_final_value_without_holding_playback_lock() {
    let finished = Arc::new(AtomicUsize::new(0));
    let animated = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    let callback_source = animated.clone();
    let callback_count = Arc::clone(&finished);
    let animated = animated.on_finish(move || {
        assert_eq!(callback_source.value(), 1.0);
        callback_source.stop();
        callback_count.fetch_add(1, Ordering::SeqCst);
    });
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));

    assert_eq!(tree.update_animations(1.0).len(), 1);
    assert_eq!(finished.load(Ordering::SeqCst), 1);
    assert!(tree.update_animations(1.0).is_empty());
    assert_eq!(finished.load(Ordering::SeqCst), 1);
}

#[test]
fn animated_finish_callback_survives_stop_but_not_playback_replacement() {
    let stopped_then_restarted = Arc::new(AtomicUsize::new(0));
    let callback_count = Arc::clone(&stopped_then_restarted);
    let animated = Animated::new(0.0_f32)
        .to(1.0, 1.0, Easing::linear)
        .on_finish(move || {
            callback_count.fetch_add(1, Ordering::SeqCst);
        });
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));

    animated.stop();
    assert_eq!(stopped_then_restarted.load(Ordering::SeqCst), 0);
    animated.restart();
    assert_eq!(tree.update_animations(1.0).len(), 1);
    assert_eq!(stopped_then_restarted.load(Ordering::SeqCst), 1);

    let stale = Arc::new(AtomicUsize::new(0));
    let stale_count = Arc::clone(&stale);
    animated.animate_to(2.0, 1.0, Easing::linear);
    animated.set_on_finish(move || {
        stale_count.fetch_add(1, Ordering::SeqCst);
    });
    animated.animate_to(3.0, 1.0, Easing::linear);
    let current = Arc::new(AtomicUsize::new(0));
    let current_count = Arc::clone(&current);
    animated.set_on_finish(move || {
        current_count.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(tree.update_animations(1.0).len(), 1);
    assert_eq!(stale.load(Ordering::SeqCst), 0);
    assert_eq!(current.load(Ordering::SeqCst), 1);
}

#[test]
fn animated_finish_callback_waits_for_delay_and_all_counted_plays() {
    let delayed_finished = Arc::new(AtomicUsize::new(0));
    let delayed_count = Arc::clone(&delayed_finished);
    let delayed = Animated::new(0.0_f32)
        .to_after(1.0, 1.0, 0.0, Easing::linear)
        .on_finish(move || {
            delayed_count.fetch_add(1, Ordering::SeqCst);
        });
    let mut delayed_tree = ViewAdapter::build_nodes(animated_root(&delayed));
    let deadline = delayed_tree.animated_source_registrations()[0]
        .1
        .expect("delayed callback deadline");
    let waiting = delayed_tree.update_animations_at(deadline - Duration::from_millis(1), 1.0);
    assert_eq!(waiting.len(), 1);
    assert!(!waiting[0].1);
    assert_eq!(delayed_finished.load(Ordering::SeqCst), 0);
    assert_eq!(delayed_tree.update_animations_at(deadline, 0.0).len(), 1);
    assert_eq!(delayed_finished.load(Ordering::SeqCst), 1);

    let counted_finished = Arc::new(AtomicUsize::new(0));
    let counted_count = Arc::clone(&counted_finished);
    let counted = Animated::new(0.0_f32)
        .to(1.0, 1.0, Easing::linear)
        .loop_count(2)
        .on_finish(move || {
            counted_count.fetch_add(1, Ordering::SeqCst);
        });
    let mut counted_tree = ViewAdapter::build_nodes(animated_root(&counted));
    assert_eq!(counted_tree.update_animations(1.0).len(), 1);
    assert_eq!(counted_finished.load(Ordering::SeqCst), 0);
    assert_eq!(counted_tree.update_animations(1.0).len(), 1);
    assert_eq!(counted_finished.load(Ordering::SeqCst), 1);

    let immediate_finished = Arc::new(AtomicUsize::new(0));
    let immediate_count = Arc::clone(&immediate_finished);
    let _ = Animated::new(0.0_f32)
        .to(1.0, 0.0, Easing::linear)
        .on_finish(move || {
            immediate_count.fetch_add(1, Ordering::SeqCst);
        });
    assert_eq!(immediate_finished.load(Ordering::SeqCst), 1);
}

#[test]
fn animation_group_rejects_non_finite_timeline_items() {
    assert_eq!(
        AnimationGroup::parallel([]).expect_err("empty group should fail"),
        AnimationGroupError::Empty
    );

    let inactive = Animated::new(0.0_f32);
    assert_eq!(
        AnimationGroup::parallel([inactive.group_item()])
            .expect_err("resting value has no group playback"),
        AnimationGroupError::InactiveItem(0)
    );

    let duplicated = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    assert_eq!(
        AnimationGroup::sequential([duplicated.group_item(), duplicated.group_item()])
            .expect_err("one source must not be scheduled twice"),
        AnimationGroupError::DuplicateItem {
            first: 0,
            duplicate: 1,
        }
    );

    let delayed = Animated::new(0.0_f32).to_after(1.0, 1.0, 1.0, Easing::linear);
    assert_eq!(
        AnimationGroup::parallel([delayed.group_item()])
            .expect_err("pre-delayed item should be explicit"),
        AnimationGroupError::DelayedItem(0)
    );

    let spring = Animated::new(0.0_f32).to_spring(1.0, Spring::snappy());
    assert_eq!(
        AnimationGroup::parallel([spring.group_item()])
            .expect_err("spring has no finite group duration"),
        AnimationGroupError::UnboundedItem(0)
    );

    let repeating = Animated::new(0.0_f32)
        .to(1.0, 1.0, Easing::linear)
        .loop_forever();
    assert_eq!(
        AnimationGroup::parallel([repeating.group_item()])
            .expect_err("infinite loop has no finite group duration"),
        AnimationGroupError::UnboundedItem(0)
    );

    let zero = Animated::new(0.0_f32).to(1.0, 0.0, Easing::linear);
    let group = AnimationGroup::sequential([zero.group_item(), AnimationGroup::delay(0.0)])
        .expect("zero-duration items are finite");
    assert_eq!(group.duration(), 0.0);
    assert_eq!(group.progress(), 1.0);
    assert!(group.is_finished());

    let zero_interval_first = Animated::new(0.0_f32).to(1.0, 1.0, Easing::linear);
    let zero_interval_second = Animated::new(0.0_f32).to(1.0, 2.0, Easing::linear);
    let normalized = AnimationGroup::stagger(
        [
            zero_interval_first.group_item(),
            zero_interval_second.group_item(),
        ],
        f64::INFINITY,
    )
    .expect("non-finite stagger interval normalizes to zero");
    assert_eq!(normalized.duration(), 2.0);
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
fn spring_parameters_preserve_valid_values_and_reject_non_converging_inputs() {
    let configured = Spring::custom()
        .stiffness(200.0)
        .damping(15.0)
        .mass(2.0)
        .velocity(-0.5)
        .rest_speed(0.01)
        .rest_displacement(0.02);
    assert_eq!(configured.stiffness_value(), 200.0);
    assert_eq!(configured.damping_value(), 15.0);
    assert_eq!(configured.mass_value(), 2.0);
    assert_eq!(configured.initial_velocity(), -0.5);
    assert_eq!(configured.rest_speed_value(), 0.01);
    assert_eq!(configured.rest_displacement_value(), 0.02);

    let normalized = configured
        .stiffness(-1.0)
        .damping(0.0)
        .mass(f64::INFINITY)
        .velocity(f64::NAN)
        .rest_speed(-1.0)
        .rest_displacement(0.0);
    assert_eq!(normalized, configured);

    let critical = Spring::critical();
    let ratio = critical.damping_value()
        / (2.0 * (critical.stiffness_value() * critical.mass_value()).sqrt());
    assert!((ratio - 1.0).abs() < 1e-12);
}

#[test]
fn spring_animation_overshoots_then_converges_and_fires_finish_once() {
    let finished = Arc::new(AtomicUsize::new(0));
    let callback_count = Arc::clone(&finished);
    let mut animation = SpringAnimation::new(0.0_f64, 1.0, Spring::bouncy()).on_finish(move || {
        callback_count.fetch_add(1, Ordering::SeqCst);
    });
    let mut overshot = false;

    for _ in 0..600 {
        overshot |= animation.update(1.0 / 60.0) > 1.0;
        if animation.is_finished() {
            break;
        }
    }

    assert!(overshot);
    assert!(animation.is_finished());
    assert_eq!(animation.value(), 1.0);
    assert_eq!(animation.progress(), 1.0);
    assert_eq!(animation.velocity(), 0.0);
    assert_eq!(finished.load(Ordering::SeqCst), 1);
    let _ = animation.update(1.0);
    assert_eq!(finished.load(Ordering::SeqCst), 1);
}

#[test]
fn critical_spring_pauses_without_progress_and_converges_without_overshoot() {
    let mut animation = SpringAnimation::new(0.0_f32, 100.0, Spring::critical());
    let first = animation.update(0.05);
    assert!(first > 0.0);
    assert!(first < 100.0);

    animation.pause();
    assert_eq!(animation.update(10.0), first);
    assert!(!animation.is_finished());

    animation.resume();
    let mut previous = first;
    for _ in 0..600 {
        let value = animation.update(1.0 / 60.0);
        assert!(value + 1e-4 >= previous);
        assert!(value <= 100.0 + 1e-4);
        previous = value;
        if animation.is_finished() {
            break;
        }
    }

    assert!(animation.is_finished());
    assert_eq!(animation.value(), 100.0);
}

#[test]
fn spring_animation_converges_for_descending_scalar_values() {
    let mut animation = SpringAnimation::new(10.0_f64, -5.0, Spring::snappy());
    assert!(!animation.is_finished());

    for _ in 0..600 {
        let _ = animation.update(1.0 / 60.0);
        if animation.is_finished() {
            break;
        }
    }

    assert!(animation.is_finished());
    assert_eq!(animation.value(), -5.0);
}

#[test]
fn animated_spring_reuses_window_registration_and_stops_at_rest() {
    let animated = Animated::new(0.0_f32).to_spring(1.0, Spring::snappy());
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));

    let updates = tree.update_animations(0.0);
    assert_eq!(updates.len(), 1);
    assert!(updates[0].1);
    assert_eq!(animated.value(), 0.0);

    let _ = tree.update_animations(0.05);
    let before_pause = animated.value();
    assert!(before_pause > 0.0);
    animated.pause();
    assert!(tree.update_animations(1.0).is_empty());
    assert_eq!(animated.value(), before_pause);

    animated.resume();
    for _ in 0..600 {
        let updates = tree.update_animations(1.0 / 60.0);
        if updates.iter().any(|(_, active)| !active) {
            break;
        }
    }

    assert!(animated.is_finished());
    assert_eq!(animated.value(), 1.0);
    assert!(tree.update_animations(1.0 / 60.0).is_empty());
}

#[test]
fn animated_spring_retargets_from_the_current_value() {
    let animated = Animated::new(0.0_f32).to_spring(1.0, Spring::gentle());
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));
    let _ = tree.update_animations(0.1);
    let current = animated.value();

    animated.animate_to_spring(2.0, Spring::snappy());
    assert_eq!(animated.value(), current);
    let _ = tree.update_animations(0.05);
    assert!(animated.value() > current);

    for _ in 0..600 {
        let updates = tree.update_animations(1.0 / 60.0);
        if updates.iter().any(|(_, active)| !active) {
            break;
        }
    }
    assert!(animated.is_finished());
    assert_eq!(animated.value(), 2.0);
}

#[test]
fn keyframes_sort_clamp_and_keep_the_last_duplicate_declaration() {
    let animation = KeyframeAnimation::new(
        [
            Keyframe::new(1.0, 9.0_f64),
            Keyframe::new(0.5, 5.0).easing(Easing::ease_in),
            Keyframe::new(-1.0, 0.0),
            Keyframe::new(0.5, 7.0).easing(Easing::ease_out),
            Keyframe::new(2.0, 10.0),
        ],
        1.0,
    )
    .expect("finite non-empty keyframes should normalize");

    assert_eq!(animation.frames().len(), 3);
    assert_eq!(animation.frames()[0], Keyframe::new(0.0, 0.0));
    assert_eq!(animation.frames()[1].offset, 0.5);
    assert_eq!(animation.frames()[1].value, 7.0);
    assert_eq!(animation.frames()[1].easing, Easing::ease_out);
    assert_eq!(animation.frames()[2], Keyframe::new(1.0, 10.0));

    assert_eq!(
        KeyframeAnimation::<f32>::new([], 1.0).expect_err("empty sequence should fail"),
        KeyframeError::Empty
    );
    assert_eq!(
        KeyframeAnimation::new([Keyframe::new(f64::NAN, 0.0_f32)], 1.0)
            .expect_err("non-finite offset should fail"),
        KeyframeError::NonFiniteOffset
    );
}

#[test]
fn keyframe_animation_applies_segment_easing_and_large_delta_completion() {
    let finished = Arc::new(AtomicUsize::new(0));
    let callback_count = Arc::clone(&finished);
    let mut animation = KeyframeAnimation::new(
        [
            Keyframe::new(0.0, 0.0_f64).easing(Easing::ease_in),
            Keyframe::new(0.5, 10.0),
            Keyframe::new(1.0, 20.0),
        ],
        1.0,
    )
    .expect("keyframe sequence")
    .on_finish(move || {
        callback_count.fetch_add(1, Ordering::SeqCst);
    });

    assert!((animation.update(0.25) - 2.5).abs() < 1e-9);
    assert!((animation.update(0.5) - 15.0).abs() < 1e-9);
    assert_eq!(animation.update(10.0), 20.0);
    assert!(animation.is_finished());
    assert_eq!(animation.progress(), 1.0);
    assert_eq!(finished.load(Ordering::SeqCst), 1);
    let _ = animation.update(1.0);
    assert_eq!(finished.load(Ordering::SeqCst), 1);
}

#[test]
fn keyframe_animation_pause_resume_reverse_and_stop_preserve_direction() {
    let mut animation =
        KeyframeAnimation::new([Keyframe::new(0.0, 0.0_f32), Keyframe::new(1.0, 20.0)], 1.0)
            .expect("keyframe sequence");

    assert_eq!(animation.update(0.25), 5.0);
    animation.pause();
    assert_eq!(animation.update(10.0), 5.0);
    assert!(!animation.is_finished());
    animation.resume();
    assert_eq!(animation.update(0.25), 10.0);

    animation.reverse();
    assert_eq!(animation.value(), 20.0);
    assert_eq!(animation.update(0.25), 15.0);
    animation.stop();
    assert_eq!(animation.value(), 0.0);
    assert!(animation.is_finished());
}

#[test]
fn animated_keyframes_use_current_value_for_a_missing_zero_boundary() {
    let animated = Animated::new(2.0_f32)
        .to_keyframes([Keyframe::new(0.5, 10.0), Keyframe::new(1.0, 20.0)], 1.0)
        .expect("keyframe sequence");
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));

    let updates = tree.update_animations(0.0);
    assert_eq!(updates.len(), 1);
    assert!(updates[0].1);
    assert_eq!(animated.value(), 2.0);

    let _ = tree.update_animations(0.25);
    assert!((animated.value() - 6.0).abs() < 1e-6);
    animated.pause();
    assert!(tree.update_animations(1.0).is_empty());
    assert!((animated.value() - 6.0).abs() < 1e-6);

    animated.resume();
    let updates = tree.update_animations(0.75);
    assert_eq!(updates.len(), 1);
    assert!(!updates[0].1);
    assert_eq!(animated.value(), 20.0);
    assert!(animated.is_finished());
    assert!(tree.update_animations(0.1).is_empty());
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
fn delayed_animated_consumes_time_past_a_late_deadline() {
    let animated = Animated::new(0.0_f32).to_after(1.0, 1.0, 1.0, Easing::linear);
    let mut tree = ViewAdapter::build_nodes(animated_root(&animated));
    let deadline = tree.animated_source_registrations()[0]
        .1
        .expect("delayed animation deadline");

    let updates = tree.update_animations_at(deadline + Duration::from_millis(500), 1.5);
    assert_eq!(updates.len(), 1);
    assert!(updates[0].1);
    assert!((animated.value() - 0.5).abs() < 0.01);
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
