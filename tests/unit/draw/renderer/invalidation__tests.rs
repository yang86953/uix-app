use super::{Invalidation, InvalidationQueue};
use crate::core::Rect;
use crate::draw::scene::NodeId;

// 局部 Paint 数量增长时，全帧与绘制判定仍只读取队列维护的聚合事实。
#[test]
fn localized_paint_keeps_constant_time_queue_flags_consistent() {
    let mut queue = InvalidationQueue::new();
    for raw_id in 1..=1_024 {
        queue.push(Invalidation::Paint {
            id: NodeId::new(raw_id),
            rect: Some(Rect::new(raw_id as f32, 0.0, 1.0, 1.0)),
        });
    }

    assert!(queue.has_paint_or_composite());
    assert!(!queue.needs_full_frame());
    assert!(queue.node_needs_paint(NodeId::new(1_024)));
    assert!(!queue.node_needs_paint(NodeId::new(2_048)));
}

// 同一节点从局部 Paint 升级为全帧 Paint 后，聚合事实必须同步升级。
#[test]
fn merged_paint_upgrade_sets_full_frame_flag() {
    let mut queue = InvalidationQueue::new();
    let id = NodeId::new(7);
    queue.push(Invalidation::Paint {
        id,
        rect: Some(Rect::new(1.0, 2.0, 3.0, 4.0)),
    });
    queue.push(Invalidation::Paint { id, rect: None });

    assert!(queue.needs_full_frame());
    assert!(queue.node_needs_paint(NodeId::new(99)));
}

// 只消费 Layout 不得丢失绘制事实；完整清理必须同时复位全部聚合状态。
#[test]
fn layout_and_full_clear_preserve_queue_flag_lifecycle() {
    let mut queue = InvalidationQueue::new();
    queue.push(Invalidation::Layout(NodeId::new(1)));
    queue.push(Invalidation::Composite {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        scroll: None,
    });

    queue.clear_layout();
    assert!(!queue.has_layout());
    assert!(queue.has_paint_or_composite());
    assert!(!queue.needs_full_frame());

    queue.clear();
    assert!(queue.is_empty());
    assert!(!queue.has_paint_or_composite());
    assert!(!queue.needs_full_frame());
}
