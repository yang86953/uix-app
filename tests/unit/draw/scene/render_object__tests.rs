use std::cell::Cell;
use std::collections::HashSet;

use super::RenderObjectTree;
use crate::core::{DirtyRegion, Point, Rect};
use crate::draw::painting::PaintContext;
use crate::draw::scene::{NodeId, ScenePaint};

const ROOT: NodeId = NodeId::new(1);
const CHILD: NodeId = NodeId::new(2);
const ROOT_CHILDREN: &[NodeId] = &[CHILD];

// 提供可切换批量快照能力的最小场景，计数逐节点查询是否被安全跳过。
struct SnapshotScene {
    dirty_nodes: HashSet<NodeId>,
    full_paint: bool,
    supports_snapshot: bool,
    child_y: f32,
    node_dirty_calls: Cell<usize>,
}

impl SnapshotScene {
    fn new(dirty_nodes: impl IntoIterator<Item = NodeId>, supports_snapshot: bool) -> Self {
        Self {
            dirty_nodes: dirty_nodes.into_iter().collect(),
            full_paint: false,
            supports_snapshot,
            child_y: 2.0,
            node_dirty_calls: Cell::new(0),
        }
    }
}

impl ScenePaint for SnapshotScene {
    fn root_id(&self) -> Option<NodeId> {
        Some(ROOT)
    }

    fn tree_version(&self) -> u64 {
        1
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::empty()
    }

    fn node_visible(&self, _id: NodeId) -> bool {
        true
    }

    fn node_frame(&self, id: NodeId) -> Rect {
        let y = if id == CHILD {
            self.child_y
        } else {
            id.slot() as f32
        };
        Rect::new(0.0, y, 10.0, 10.0)
    }

    fn node_dirty(&self, id: NodeId) -> bool {
        self.node_dirty_calls
            .set(self.node_dirty_calls.get().saturating_add(1));
        self.full_paint || self.dirty_nodes.contains(&id)
    }

    fn paint_invalidation_snapshot_into(&self, ids: &mut HashSet<NodeId>) -> Option<bool> {
        ids.clear();
        if !self.supports_snapshot {
            return None;
        }
        ids.extend(self.dirty_nodes.iter().copied());
        Some(self.full_paint)
    }

    fn node_z_index(&self, _id: NodeId) -> i32 {
        0
    }

    fn node_children(&self, id: NodeId) -> &[NodeId] {
        if id == ROOT { ROOT_CHILDREN } else { &[] }
    }

    fn children_clip(&self, _id: NodeId, _frame: Rect) -> Option<Rect> {
        None
    }

    fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
        frame
    }

    fn scroll_offset(&self, _id: NodeId) -> Option<(f32, f32)> {
        None
    }

    fn focused_node(&self) -> Option<NodeId> {
        None
    }

    fn node_focusable(&self, _id: NodeId) -> bool {
        false
    }

    fn hit_test(&self, _pos: Point) -> Option<NodeId> {
        None
    }

    fn parent(&self, id: NodeId) -> Option<NodeId> {
        (id == CHILD).then_some(ROOT)
    }

    fn paint(&self, _id: NodeId, _frame: Rect, _ctx: &mut PaintContext<'_>) {}
}

// 批量局部快照必须标记精确节点，且同步阶段不再逐节点调用 node_dirty。
#[test]
fn batch_snapshot_marks_only_declared_nodes_without_per_node_queries() {
    let scene = SnapshotScene::new([CHILD], true);
    let mut tree = RenderObjectTree::new();
    tree.sync(&scene);
    for entry in tree.entries.values_mut() {
        entry.is_dirty = false;
    }

    tree.sync(&scene);

    assert!(!tree.get(ROOT).expect("根节点必须存在").is_dirty);
    assert!(tree.get(CHILD).expect("子节点必须存在").is_dirty);
    assert_eq!(scene.node_dirty_calls.get(), 0);
}

// 不支持批量快照的 ScenePaint 必须保留原有逐节点查询语义。
#[test]
fn unsupported_snapshot_falls_back_to_per_node_queries() {
    let scene = SnapshotScene::new([CHILD], false);
    let mut tree = RenderObjectTree::new();
    tree.sync(&scene);
    for entry in tree.entries.values_mut() {
        entry.is_dirty = false;
    }
    scene.node_dirty_calls.set(0);

    tree.sync(&scene);

    assert!(!tree.get(ROOT).expect("根节点必须存在").is_dirty);
    assert!(tree.get(CHILD).expect("子节点必须存在").is_dirty);
    assert_eq!(scene.node_dirty_calls.get(), 2);
}

// 全帧快照必须保守标记全部条目，不能依赖局部身份集合。
#[test]
fn full_snapshot_marks_every_render_object_dirty() {
    let mut scene = SnapshotScene::new([], true);
    let mut tree = RenderObjectTree::new();
    tree.sync(&scene);
    for entry in tree.entries.values_mut() {
        entry.is_dirty = false;
    }
    scene.full_paint = true;

    tree.sync(&scene);

    assert!(tree.get(ROOT).expect("根节点必须存在").is_dirty);
    assert!(tree.get(CHILD).expect("子节点必须存在").is_dirty);
    assert_eq!(scene.node_dirty_calls.get(), 0);
}

// 批量 Paint 快照为空时，frame 变化仍必须独立使对应 RenderObject 失效。
#[test]
fn batch_snapshot_does_not_hide_frame_changes() {
    let mut scene = SnapshotScene::new([], true);
    let mut tree = RenderObjectTree::new();
    tree.sync(&scene);
    for entry in tree.entries.values_mut() {
        entry.is_dirty = false;
    }
    scene.child_y = 24.0;

    tree.sync(&scene);

    assert!(!tree.get(ROOT).expect("根节点必须存在").is_dirty);
    assert!(tree.get(CHILD).expect("子节点必须存在").is_dirty);
    assert_eq!(tree.get(CHILD).expect("子节点必须存在").frame.y, 24.0);
    assert_eq!(scene.node_dirty_calls.get(), 0);
}
