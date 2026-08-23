use super::*;
use crate::core::{DirtyRegion, Point, Rect, WidgetId};
use crate::draw::painting::PaintContext;

const ROOT_NODE: NodeId = WidgetId::new(1);
const OVERLAY_NODE: NodeId = WidgetId::new(2);

// 构造位于滚动视口中的最小浮层场景。
struct ScrolledOverlayScene;

impl ScenePaint for ScrolledOverlayScene {
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
        if id == OVERLAY_NODE {
            Rect::new(10.0, 100.0, 20.0, 10.0)
        } else {
            Rect::new(0.0, 0.0, 100.0, 100.0)
        }
    }

    fn node_dirty(&self, _id: NodeId) -> bool {
        false
    }

    fn node_z_index(&self, id: NodeId) -> i32 {
        if id == OVERLAY_NODE { 900 } else { 0 }
    }

    fn node_children(&self, id: NodeId) -> &[NodeId] {
        static CHILDREN: [NodeId; 1] = [OVERLAY_NODE];
        if id == ROOT_NODE { &CHILDREN } else { &[] }
    }

    fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect> {
        (id == ROOT_NODE).then_some(frame)
    }

    fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
        // 测试场景没有超出布局边界的绘制效果。
        frame
    }

    fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
        (id == ROOT_NODE).then_some((0.0, 80.0))
    }

    fn node_is_overlay(&self, id: NodeId) -> bool {
        id == OVERLAY_NODE
    }

    fn node_overlay_transform(&self, id: NodeId) -> crate::draw::Transform {
        crate::draw::scene::viewport_transform::overlay_root_visual_transform(self, id)
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
        (id == OVERLAY_NODE).then_some(ROOT_NODE)
    }

    fn paint(&self, _id: NodeId, _frame: Rect, _ctx: &mut PaintContext<'_>) {}
}

#[test]
fn overlay_root_keeps_ancestor_scroll_transform_across_updates() {
    let scene = ScrolledOverlayScene;
    let mut tree = LayerTree::new();

    tree.build(&scene, false);
    assert_eq!(tree.overlays.len(), 1);

    let expected = Rect::new(10.0, 20.0, 20.0, 10.0);
    assert_eq!(
        crate::draw::scene::viewport_transform::node_visual_rect(
            &scene,
            OVERLAY_NODE,
            scene.node_frame(OVERLAY_NODE),
        ),
        expected
    );
    let overlay = &tree.overlays[0];
    assert_eq!(
        overlay
            .transform()
            .transform_rect(scene.node_frame(OVERLAY_NODE)),
        expected
    );

    // 增量脏状态同步不得把浮层根重新置回未滚动的内容坐标。
    tree.update_dirty(&scene);
    let overlay = &tree.overlays[0];
    assert_eq!(
        overlay
            .transform()
            .transform_rect(scene.node_frame(OVERLAY_NODE)),
        expected
    );
}
