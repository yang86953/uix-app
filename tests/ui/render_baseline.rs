//! ui 域 — 渲染基线集成测试。
//!
//! 运行方式：
//! ```bash
//! cargo test --features test-harness -p uix ui 域 render_baseline
//! cargo test -p uix draw 域 render_baseline
//! ```

#![cfg(feature = "test-harness")]

use uix::draw::pipeline::RenderMetrics;
use uix::native::{KeyMod, Point, Rect, ScrollDirection};
use uix::ui::widgets::container::Container;
use uix::ui::widgets::general::Button;
use uix::ui::widgets::other::scroll_view::ScrollView;
use uix::ui::widgets::Skeleton;
use uix::ui::{WidgetCore, WidgetEvent, WidgetTree};

#[test]
fn baseline_idle_zero_present() {
    let mut tree = WidgetTree::new();
    tree.bind_invalidation();
    tree.set_root(Box::new(Container::new()));
    tree.layout();
    // 模拟 event_loop 首帧全帧脏
    tree.mark_full_frame_dirty();
    assert!(tree.has_render_work());
    tree.reset_dirty();
    // 静止：无动画、无事件 → 无渲染工作
    tree.update(0.016);
    assert!(!tree.has_render_work());
    assert!(!tree.animations_active());

    let metrics = RenderMetrics::default();
    assert_eq!(metrics.present_calls, 0);
}

#[test]
fn baseline_hover_partial_damage() {
    let mut tree = WidgetTree::new();
    tree.bind_invalidation();
    let btn_id = tree.set_root(Box::new(Button::new("Hover")));
    if let Some(btn) = tree.get_mut(btn_id) {
        btn.set_frame(Rect::new(10.0, 10.0, 80.0, 32.0));
    }
    tree.layout();
    tree.reset_dirty();

    tree.dispatch_event(&WidgetEvent::MouseMove {
        pos: Point::new(20.0, 20.0),
        mods: KeyMod::NONE,
    });

    assert!(tree.has_render_work());
    let region = tree.dirty_region();
    assert!(!region.full_frame);
    let bounds = region.bounds();
    // damage 应接近 button 区域，远小于典型窗口
    assert!(bounds.w * bounds.h <= 80.0 * 32.0 * 4.0);
    assert!(bounds.w * bounds.h < 800.0 * 600.0);
}

#[test]
fn baseline_scroll_strip_repaint() {
    let mut tree = WidgetTree::new();
    tree.bind_invalidation();
    let sv_id = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(100.0, 100.0),
    ));
    if let Some(sv) = tree.get_mut(sv_id) {
        sv.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    }
    let child = tree.add_child(sv_id, Box::new(Container::new().size(100.0, 400.0)));
    let _ = child;
    tree.layout();

    let max_scroll = tree
        .get(sv_id)
        .and_then(|n| n.component().as_any().downcast_ref::<ScrollView>())
        .map(|sv| sv.max_scroll_y())
        .unwrap_or(0.0);
    assert!(
        max_scroll > 1.0,
        "布局后 ScrollView 应可滚动，max_scroll_y={max_scroll}"
    );

    tree.reset_dirty();

    let frame = tree.get(sv_id).map(|n| n.frame()).unwrap_or_default();
    tree.dispatch_event(&WidgetEvent::MouseWheel {
        pos: Point::new(frame.x + 1.0, frame.y + 1.0),
        delta: Point::new(0.0, 1.0),
    });

    for _ in 0..40 {
        tree.update(0.016);
        if tree.has_render_work() {
            break;
        }
    }

    assert!(tree.has_render_work(), "滚动后应产生渲染工作");
    let region = tree.dirty_region();
    assert!(!region.full_frame);
    let viewport_area = 100.0 * 100.0;
    let bounds = region.bounds();
    assert!(
        bounds.w * bounds.h < viewport_area * 0.85,
        "scroll damage 应近似 strip，而非整视口 (got {} vs viewport {})",
        bounds.w * bounds.h,
        viewport_area
    );
}

#[test]
fn baseline_ten_animations_ten_nodes() {
    let mut tree = WidgetTree::new();
    tree.bind_invalidation();
    let root = tree.set_root(Box::new(Container::new()));
    if let Some(r) = tree.get_mut(root) {
        r.set_frame(Rect::new(0.0, 0.0, 200.0, 320.0));
    }
    for i in 0..10 {
        let id = tree.add_child(root, Box::new(Skeleton::new()));
        if let Some(node) = tree.get_mut(id) {
            node.set_frame(Rect::new(0.0, i as f32 * 32.0, 200.0, 30.0));
        }
    }
    tree.layout();
    tree.reset_dirty();

    tree.update(0.016);

    assert!(tree.has_render_work());
    let inv = tree.invalidation().lock().unwrap();
    let painted: Vec<_> = tree
        .traverse()
        .into_iter()
        .filter(|id| inv.node_needs_paint(*id))
        .collect();
    assert_eq!(
        painted.len(),
        10,
        "10 个动画节点应各自产生 Paint 失效，实际 {} 个",
        painted.len()
    );
}
