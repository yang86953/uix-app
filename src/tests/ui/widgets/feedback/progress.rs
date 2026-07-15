use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::ProgressMode;
use crate::ui::{AccessibilityRole, ProgressBar};

#[test]
fn progress_bar_advertises_animation_capability() {
    let progress = ProgressBar::new().indeterminate();

    assert!(progress
        .capabilities()
        .contains(WidgetCapabilities::ANIMATION));
    assert!(progress.as_animation().is_some());
}

#[test]
fn indeterminate_progress_advances_phase_and_marks_paint_dirty() {
    let mut progress = ProgressBar::new().indeterminate();
    let initial_phase = progress.animation_phase();

    assert!(WidgetAnimation::update_animation(&mut progress, 0.25));
    assert_ne!(progress.animation_phase(), initial_phase);
    let frame = Rect::new(4.0, 5.0, 120.0, 8.0);
    let dirty = WidgetAnimation::dirty_bounds(&progress, frame);
    assert_eq!(dirty.y, frame.y);
    assert_eq!(dirty.h, frame.h);
    assert!(dirty.w < frame.w);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(ProgressBar::new().indeterminate()));
    let root = tree.get_mut(id).expect("progress root");
    root.set_frame(frame);
    root.set_active(true);
    tree.invalidation().lock().unwrap().clear();

    assert!(tree.update(1.0 / 60.0));

    let queue = tree.invalidation().lock().unwrap();
    assert!(queue.has_paint_or_composite());
    assert!(queue.node_needs_paint(id));
    let region = queue.dirty_region();
    assert_eq!(region.rects().len(), 1);
    assert!(region.rects()[0].w < frame.w);
}

#[test]
fn determinate_progress_does_not_keep_animation_pending() {
    let mut progress = ProgressBar::new().progress(0.4);
    let initial_phase = progress.animation_phase();

    assert!(!WidgetAnimation::update_animation(&mut progress, 0.25));
    assert_eq!(progress.animation_phase(), initial_phase);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(ProgressBar::new().progress(0.4)));
    let root = tree.get_mut(id).expect("progress root");
    root.set_frame(Rect::new(4.0, 5.0, 120.0, 8.0));
    root.set_active(true);
    tree.invalidation().lock().unwrap().clear();

    assert!(!tree.update(1.0 / 60.0));
    assert!(tree.invalidation().lock().unwrap().is_empty());
}

#[test]
fn progress_normalizes_non_finite_values_and_dimensions() {
    let progress = ProgressBar::new()
        .progress(f32::NAN)
        .size(f32::INFINITY, -10.0)
        .round(false);

    assert!(matches!(
        progress.snapshot_fields(),
        SnapshotFields::ProgressBar {
            progress: 0.0,
            mode: ProgressMode::Determinate(0.0),
            width: 0.0,
            height: 0.0,
            round: false,
            ..
        }
    ));
    assert_eq!(
        progress.measure(Constraints::loose(Size::new(200.0, 40.0))),
        Size::zero()
    );
}

#[test]
fn indeterminate_accessibility_does_not_invent_a_numeric_value() {
    let indeterminate = ProgressBar::new().indeterminate();
    let accessibility = indeterminate.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::ProgressBar);
    assert_eq!(accessibility.state.value_now, None);
    assert_eq!(accessibility.state.value_min, None);
    assert_eq!(accessibility.state.value_max, None);

    let determinate = ProgressBar::new().progress(0.65);
    let accessibility = determinate.snapshot_fields().accessibility();
    assert!(accessibility
        .state
        .value_now
        .is_some_and(|value| (value - 0.65).abs() < 1.0e-6));
    assert_eq!(accessibility.state.value_min, Some(0.0));
    assert_eq!(accessibility.state.value_max, Some(1.0));
}
