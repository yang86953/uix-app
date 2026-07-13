use crate::tests::common::*;
use crate::component;
use crate::draw::{ Radius };
use crate::ui::core::widget::WidgetCore;
use crate::ui::{ ProgressBar };

#[test]
fn progress_bar_advertises_animation_capability() {
    let progress = ProgressBar::new().indeterminate();

    assert!(progress
        .capabilities()
        .contains(WidgetCapabilities::ANIMATION));
    assert!(progress.as_animation().is_some());
}

#[test]
fn measure_clamps_progress_size() {
    let measured = ProgressBar::new()
        .size(200.0, 8.0)
        .measure(Constraints::loose(Size::new(120.0, 6.0)));

    assert_eq!(measured, Size::new(120.0, 6.0));
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
