use super::needs_paint_in_viewport;
use crate::core::{DirtyRegion, Point, Rect, WidgetId};
use crate::draw::painting::PaintContext;
use crate::draw::scene::{NodeId, ScenePaint};
use std::cell::Cell;

const ROOT_NODE: NodeId = WidgetId::new(1);
const CHILD_NODE: NodeId = WidgetId::new(2);

struct CullScene {
    child_frame: Rect,
    child_dirty_rect: Rect,
    child_dirty: bool,
    parent_calls: Cell<usize>,
}

impl ScenePaint for CullScene {
    fn root_id(&self) -> Option<NodeId> {
        Some(ROOT_NODE)
    }

    fn tree_version(&self) -> u64 {
        1
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::full()
    }

    fn node_visible(&self, _id: NodeId) -> bool {
        true
    }

    fn node_frame(&self, id: NodeId) -> Rect {
        if id == CHILD_NODE {
            self.child_frame
        } else {
            Rect::new(0.0, 0.0, 100.0, 100.0)
        }
    }

    fn node_dirty(&self, id: NodeId) -> bool {
        id == CHILD_NODE && self.child_dirty
    }

    fn node_z_index(&self, _id: NodeId) -> i32 {
        0
    }

    fn node_children(&self, id: NodeId) -> &[NodeId] {
        static CHILDREN: [NodeId; 1] = [CHILD_NODE];
        if id == ROOT_NODE { &CHILDREN } else { &[] }
    }

    fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect> {
        (id == ROOT_NODE).then_some(frame)
    }

    fn dirty_rect(&self, id: NodeId, frame: Rect) -> Rect {
        if id == CHILD_NODE {
            self.child_dirty_rect
        } else {
            frame
        }
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
        self.parent_calls.set(self.parent_calls.get() + 1);
        (id == CHILD_NODE).then_some(ROOT_NODE)
    }

    fn paint(&self, _id: NodeId, _frame: Rect, _ctx: &mut PaintContext<'_>) {}
}

#[test]
fn dirty_child_fully_outside_parent_clip_is_culled() {
    let frame = Rect::new(0.0, 120.0, 20.0, 20.0);
    let scene = CullScene {
        child_frame: frame,
        child_dirty_rect: frame,
        child_dirty: true,
        parent_calls: Cell::new(0),
    };

    assert!(!needs_paint_in_viewport(
        &scene,
        CHILD_NODE,
        &DirtyRegion::full()
    ));
}

#[test]
fn dirty_rect_extension_entering_parent_clip_is_preserved() {
    let scene = CullScene {
        child_frame: Rect::new(0.0, 120.0, 20.0, 20.0),
        child_dirty_rect: Rect::new(0.0, 90.0, 20.0, 50.0),
        child_dirty: true,
        parent_calls: Cell::new(0),
    };

    assert!(needs_paint_in_viewport(
        &scene,
        CHILD_NODE,
        &DirtyRegion::full()
    ));
}

#[test]
fn clean_visible_child_still_follows_frame_damage() {
    let frame = Rect::new(10.0, 10.0, 20.0, 20.0);
    let scene = CullScene {
        child_frame: frame,
        child_dirty_rect: frame,
        child_dirty: false,
        parent_calls: Cell::new(0),
    };

    assert!(!needs_paint_in_viewport(
        &scene,
        CHILD_NODE,
        &DirtyRegion::area(Rect::new(50.0, 50.0, 10.0, 10.0))
    ));
    assert!(needs_paint_in_viewport(
        &scene,
        CHILD_NODE,
        &DirtyRegion::area(Rect::new(15.0, 15.0, 2.0, 2.0))
    ));
}

#[test]
fn visible_culling_builds_the_ancestor_path_once() {
    let frame = Rect::new(10.0, 10.0, 20.0, 20.0);
    let scene = CullScene {
        child_frame: frame,
        child_dirty_rect: frame,
        child_dirty: false,
        parent_calls: Cell::new(0),
    };

    assert!(needs_paint_in_viewport(
        &scene,
        CHILD_NODE,
        &DirtyRegion::area(frame)
    ));
    // child 和 root 各读取一次 parent；不得为投影与裁剪重复建路径。
    assert_eq!(scene.parent_calls.get(), 2);
}
