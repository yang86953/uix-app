use crate::core::Rect;
use crate::ui::traits::{WidgetAnimation, WidgetCapabilities, WidgetComponent};
use crate::ui::{ProgressBar, WidgetCore, WidgetTree};

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

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(ProgressBar::new().indeterminate()));
    tree.get_mut(id)
        .expect("progress root")
        .set_frame(Rect::new(4.0, 5.0, 120.0, 8.0));
    tree.invalidation().lock().unwrap().clear();

    assert!(tree.update(1.0 / 60.0));

    let queue = tree.invalidation().lock().unwrap();
    assert!(queue.has_paint_or_composite());
    assert!(queue.node_needs_paint(id));
}

#[test]
fn determinate_progress_does_not_keep_animation_pending() {
    let mut progress = ProgressBar::new().progress(0.4);
    let initial_phase = progress.animation_phase();

    assert!(!WidgetAnimation::update_animation(&mut progress, 0.25));
    assert_eq!(progress.animation_phase(), initial_phase);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(ProgressBar::new().progress(0.4)));
    tree.get_mut(id)
        .expect("progress root")
        .set_frame(Rect::new(4.0, 5.0, 120.0, 8.0));
    tree.invalidation().lock().unwrap().clear();

    assert!(!tree.update(1.0 / 60.0));
    assert!(tree.invalidation().lock().unwrap().is_empty());
}
