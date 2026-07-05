//! ui 域 — 渲染基线集成测试。
//!
//! 运行方式：
//! ```bash
//! cargo test --features test-harness -p uix ui 域 render_baseline
//! cargo test -p uix draw 域 render_baseline
//! ```

#![cfg(feature = "test-harness")]

use uix::draw::pipeline::RenderMetrics;
use uix::native::Rect;
use uix::ui::WidgetTree;
use uix::ui::widgets::container::Container;

#[test]
fn baseline_idle_zero_present() {
    let mut tree = WidgetTree::new();
    tree.bind_invalidation();
    tree.set_root(Box::new(Container::new()));
    tree.layout();
    // 首帧全帧脏
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
#[ignore = "TODO Phase 2: hover 应仅产生 button 局部 damage"]
fn baseline_hover_partial_damage() {
    let _ = RenderMetrics::default();
}

#[test]
#[ignore = "TODO Phase 5: scroll 应近似 strip 重绘而非视口全量"]
fn baseline_scroll_strip_repaint() {
    let _ = RenderMetrics::default();
}

#[test]
#[ignore = "TODO Phase 2: 10 个独立动画应 update/paint 各 10 节点"]
fn baseline_ten_animations_ten_nodes() {
    let mut tree = WidgetTree::new();
    tree.bind_invalidation();
    tree.mark_dirty_rect(0, Rect::new(0.0, 0.0, 10.0, 10.0));
    assert!(tree.has_render_work());
    tree.reset_dirty();
    assert!(!tree.has_render_work());
}
