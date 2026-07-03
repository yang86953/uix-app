//! Phase 2 invalidation + animation_registry 单元测试。

use uix_graphics::pipeline::{
    AnimationRegistry, Invalidation, InvalidationQueue, ScrollDelta,
};
use uix_platform::Rect;

#[test]
fn render_baseline_invalidation_queue_idle() {
    let q = InvalidationQueue::new();
    assert!(q.is_empty());
}

#[test]
fn render_baseline_animation_registry_tick_scope() {
    let mut reg = AnimationRegistry::new();
    reg.register(7);
    reg.register(9);
    assert_eq!(reg.active_ids().len(), 2);
    reg.unregister(7);
    assert_eq!(reg.active_ids(), vec![9]);
}

#[test]
fn render_baseline_paint_drives_dirty_region() {
    let mut q = InvalidationQueue::new();
    q.push(Invalidation::Paint {
        id: 1,
        rect: Some(Rect::new(1.0, 2.0, 3.0, 4.0)),
    });
    assert!(q.has_paint_or_composite());
    let region = q.dirty_region();
    assert!(!region.is_empty());
}

#[test]
fn render_baseline_composite_scroll() {
    let mut q = InvalidationQueue::new();
    q.push(Invalidation::Composite {
        rect: Rect::new(0.0, 0.0, 50.0, 50.0),
        scroll: Some(ScrollDelta { dx: 1.0, dy: 0.0 }),
    });
    assert!(q.scroll_composite().is_some());
}
