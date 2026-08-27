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

// 重复全帧合成请求只需保留一项，并在清理后允许重新登记。
#[test]
fn full_composite_requests_are_coalesced_until_clear() {
    let mut queue = InvalidationQueue::new();
    queue.push(Invalidation::FullComposite);
    queue.push(Invalidation::FullComposite);

    assert_eq!(queue.items.len(), 1);
    assert!(queue.has_paint_or_composite());

    queue.clear();
    queue.push(Invalidation::FullComposite);
    assert_eq!(queue.items.len(), 1);
}

// 根 Layout 已在队列中时，子请求不新增条目但仍推进修订号。
#[test]
fn layout_until_root_suppresses_child_when_root_is_queued() {
    let mut queue = InvalidationQueue::new();
    let root = NodeId::new(1);
    let child = NodeId::new(2);
    queue.push(Invalidation::Layout(root));
    let before_revision = queue.revision();

    assert!(!queue.push_layout_until_root(root, child));
    assert_eq!(queue.items, vec![Invalidation::Layout(root)]);
    assert_eq!(queue.revision(), before_revision.wrapping_add(1));
}

// 根请求应以一次集合插入建立快返提示，后续子请求仍只保留根条目。
#[test]
fn layout_until_root_caches_direct_root_request() {
    let mut queue = InvalidationQueue::new();
    let root = NodeId::new(1);
    let child = NodeId::new(2);

    assert!(queue.push_layout_until_root(root, root));
    let before_revision = queue.revision();
    assert!(!queue.push_layout_until_root(root, root));
    assert_eq!(queue.revision(), before_revision.wrapping_add(1));
    assert!(!queue.push_layout_until_root(root, child));
    assert_eq!(queue.items, vec![Invalidation::Layout(root)]);
}

// Paint 先入队时根条目不在零号槽位，一基提示仍必须命中真实 Layout 槽位。
#[test]
fn layout_root_hint_tracks_root_after_paint_item() {
    let mut queue = InvalidationQueue::new();
    let root = NodeId::new(1);
    let child = NodeId::new(2);
    queue.push(Invalidation::Paint {
        id: child,
        rect: None,
    });

    assert!(queue.push_layout_until_root(root, root));
    assert!(!queue.push_layout_until_root(root, child));
    assert_eq!(queue.items[1], Invalidation::Layout(root));
}

// 消费 Layout 必须同时清除根快返提示，不能压掉下一轮的子节点请求。
#[test]
fn layout_root_hint_is_cleared_with_layout_items() {
    let mut queue = InvalidationQueue::new();
    let root = NodeId::new(1);
    let child = NodeId::new(2);

    assert!(queue.push_layout_until_root(root, root));
    queue.clear_layout();
    assert!(queue.push_layout_until_root(root, child));
    assert_eq!(queue.items, vec![Invalidation::Layout(child)]);
}

// 修订号匹配的完整清理也必须复位根提示，下一轮不得沿用旧队列事实。
#[test]
fn layout_root_hint_is_cleared_with_matching_revision() {
    let mut queue = InvalidationQueue::new();
    let root = NodeId::new(1);
    let child = NodeId::new(2);

    assert!(queue.push_layout_until_root(root, root));
    assert!(queue.clear_if_revision(queue.revision()));
    assert!(queue.push_layout_until_root(root, child));
    assert_eq!(queue.items, vec![Invalidation::Layout(child)]);
}

// 同一队列切换到不同根身份时，旧提示不得抑制新树的布局请求。
#[test]
fn layout_root_hint_validates_root_identity() {
    let mut queue = InvalidationQueue::new();
    let old_root = NodeId::from_parts(1, 7);
    let new_root = NodeId::from_parts(1, 8);
    let child = NodeId::new(3);

    assert!(queue.push_layout_until_root(old_root, old_root));
    assert!(queue.push_layout_until_root(new_root, child));
    assert_eq!(
        queue.items,
        vec![Invalidation::Layout(old_root), Invalidation::Layout(child)]
    );
}

// 根 Layout 尚未存在时，首次与重复子请求都继续传播且只保留一个子条目。
#[test]
fn layout_until_root_retries_child_when_root_is_missing() {
    let mut queue = InvalidationQueue::new();
    let root = NodeId::new(1);
    let child = NodeId::new(2);
    let initial_revision = queue.revision();

    assert!(queue.push_layout_until_root(root, child));
    let after_first_revision = queue.revision();
    assert_eq!(after_first_revision, initial_revision.wrapping_add(1));
    assert!(queue.push_layout_until_root(root, child));
    assert_eq!(queue.revision(), after_first_revision.wrapping_add(1));
    assert_eq!(queue.items, vec![Invalidation::Layout(child)]);
}
