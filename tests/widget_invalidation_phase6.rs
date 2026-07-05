//! Phase 6：精确 Invalidation + State 绑定验收测试。

use uix::draw::pipeline::Invalidation;
use uix::native::Rect;
use uix::ui::state::{begin_state_capture, State};
use uix::ui::view::{column, dynamic_label, label, ViewAdapter};
use uix::ui::{WidgetCore, WidgetTree};

#[test]
fn state_bind_paint_invalidation_is_targeted() {
    let mut tree = WidgetTree::new();
    tree.bind_invalidation();
    let label_id = tree.set_root(Box::new(uix::ui::widgets::Label::new("0")));
    let state = State::new(0);
    let queue = tree.invalidation_handle();
    let damage = Rect::new(10.0, 20.0, 80.0, 18.0);
    state.bind_paint_invalidation(label_id, queue, Some(damage));
    tree.reset_dirty();

    state.set(1);

    assert!(tree.has_render_work());
    let region = tree.dirty_region();
    assert!(!region.full_frame);
    assert_eq!(region.rects().len(), 1);
    let r = region.rects()[0];
    assert!((r.x - 10.0).abs() < 1e-6);
    assert!((r.w - 80.0).abs() < 1e-6);
    assert!(tree.invalidation().lock().unwrap().node_needs_paint(label_id));
}

#[test]
fn counter_increment_only_label_damage() {
    begin_state_capture();
    let count = State::new(0);
    let label_count = count.clone();

    let section = column([
        label("Header"),
        dynamic_label(move || format!("Count: {}", label_count.get())),
    ]);

    let mut tree = ViewAdapter::build_nodes(section);
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, 200.0, 100.0));
    }
    tree.layout();
    tree.mark_full_frame_dirty();
    tree.reset_dirty();

    // 模拟 Counter +1：State 变更应仅使 dynamic_label 失效
    count.set(1);

    assert!(tree.has_render_work());
    let region = tree.dirty_region();
    assert!(!region.full_frame, "Counter +1 不应触发全帧 repaint");
    assert!(
        !region.is_empty(),
        "State 变更应产生 label 区域 damage"
    );

    let inv = tree.invalidation().lock().unwrap();
    let dynamic_label_ids: Vec<_> = tree
        .traverse()
        .into_iter()
        .filter(|id| inv.node_needs_paint(*id))
        .collect();
    assert_eq!(
        dynamic_label_ids.len(),
        1,
        "应仅有一个 widget 需要 repaint"
    );
    drop(inv);

    // 至少有一个 paint 目标，且 damage 面积远小于根 frame
    let bounds = region.bounds();
    assert!(bounds.w * bounds.h < 200.0 * 100.0 * 0.5);
}

#[test]
fn layout_invalidation_skipped_when_paint_only() {
    let mut tree = WidgetTree::new();
    tree.bind_invalidation();
    tree.set_root(Box::new(uix::ui::widgets::Label::new("static")));
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, 100.0, 50.0));
    }
    tree.layout();
    tree.mark_full_frame_dirty();
    tree.reset_dirty();

    tree.invalidate_paint(0);
    assert!(tree.layout_traverse().is_empty());
    assert!(tree.has_render_work());
}

#[test]
fn bind_invalidation_enables_layout_traverse() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(uix::ui::widgets::Label::new("child")));
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, 200.0, 100.0));
    }
    tree.layout();
    tree.reset_dirty();

    tree.bind_invalidation();
    let order = tree.layout_traverse();
    assert!(
        !order.is_empty(),
        "bind_invalidation 后应能触发 layout_traverse"
    );
}

#[test]
fn layout_only_invalidation_does_not_trigger_render_work() {
    let mut tree = WidgetTree::new();
    tree.bind_invalidation();
    tree.set_root(Box::new(uix::ui::widgets::Label::new("static")));
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, 100.0, 50.0));
    }
    tree.layout();
    tree.reset_dirty();

    // 模拟 set_root / add_child 仅推送 Layout 失效的场景
    tree.invalidation()
        .lock()
        .unwrap()
        .push(Invalidation::Layout(0));

    assert!(!tree.has_render_work());
    assert!(tree.invalidation().lock().unwrap().has_layout());
}
