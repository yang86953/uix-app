//! 分层渲染的浮层变换与裁剪坐标契约。

use uix::core::{DirtyRegion, Point, Rect, WidgetId};
use uix::draw::Transform;
use uix::draw::painting::PaintContext;
use uix::draw::scene::{NodeId, ScenePaint, node_visual_rect, visible_viewport_rect};

const ROOT_NODE: NodeId = WidgetId::new(1);
const OVERLAY_NODE: NodeId = WidgetId::new(2);
const CHILD_NODE: NodeId = WidgetId::new(3);

/// 模拟滚动容器内被提升到根画布、且自身裁剪后代的浮层。
struct ScrolledClippedOverlayScene;

impl ScenePaint for ScrolledClippedOverlayScene {
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
        match id {
            OVERLAY_NODE => Rect::new(20.0, 100.0, 100.0, 60.0),
            CHILD_NODE => Rect::new(30.0, 110.0, 40.0, 20.0),
            _ => Rect::new(0.0, 0.0, 200.0, 120.0),
        }
    }

    fn node_dirty(&self, _id: NodeId) -> bool {
        false
    }

    fn node_z_index(&self, id: NodeId) -> i32 {
        i32::from(id == OVERLAY_NODE) * 900
    }

    fn node_children(&self, id: NodeId) -> &[NodeId] {
        static ROOT_CHILDREN: [NodeId; 1] = [OVERLAY_NODE];
        static OVERLAY_CHILDREN: [NodeId; 1] = [CHILD_NODE];
        match id {
            ROOT_NODE => &ROOT_CHILDREN,
            OVERLAY_NODE => &OVERLAY_CHILDREN,
            _ => &[],
        }
    }

    fn node_is_overlay(&self, id: NodeId) -> bool {
        id == OVERLAY_NODE
    }

    fn node_overlay_transform(&self, id: NodeId) -> Transform {
        if id == OVERLAY_NODE {
            Transform::translate(0.0, -80.0)
        } else {
            Transform::identity()
        }
    }

    fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect> {
        (id == OVERLAY_NODE).then_some(frame)
    }

    fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
        frame
    }

    fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
        (id == ROOT_NODE).then_some((0.0, 80.0))
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
        match id {
            OVERLAY_NODE => Some(ROOT_NODE),
            CHILD_NODE => Some(OVERLAY_NODE),
            _ => None,
        }
    }

    fn paint(&self, _id: NodeId, _frame: Rect, _ctx: &mut PaintContext<'_>) {}
}

#[test]
fn overlay_descendant_clip_uses_the_same_root_transform_as_painting() {
    let scene = ScrolledClippedOverlayScene;
    let expected = Rect::new(30.0, 30.0, 40.0, 20.0);

    assert_eq!(
        node_visual_rect(&scene, CHILD_NODE, scene.node_frame(CHILD_NODE)),
        expected
    );
    assert_eq!(visible_viewport_rect(&scene, CHILD_NODE), Some(expected));
}
