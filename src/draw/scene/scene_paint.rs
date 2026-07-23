//! ScenePaint — 场景绘制抽象，解耦 compositor 与 ui 域。

use crate::core::{Point, Rect};

use crate::core::DirtyRegion;
use crate::draw::api::PaintContext;
use crate::draw::renderer::NodeId;
use crate::draw::Transform;

/// Picture cache eligibility declared by widget metadata and refined by runtime signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PicturePolicy {
    Never,
    Eligible,
}

/// 场景绘制契约：LayerTree 通过此 trait 读取节点元数据并下发绘制。
pub trait ScenePaint {
    fn root_id(&self) -> Option<NodeId>;
    fn tree_version(&self) -> u64;
    fn dirty_region(&self) -> DirtyRegion;
    fn node_visible(&self, id: NodeId) -> bool;
    fn node_frame(&self, id: NodeId) -> Rect;
    fn node_dirty(&self, id: NodeId) -> bool;
    fn node_z_index(&self, id: NodeId) -> i32;
    /// Visual transform for this node and its descendants, in layout coordinates.
    fn node_transform(&self, id: NodeId) -> Transform {
        let _ = id;
        Transform::identity()
    }
    /// Opacity multiplier for this node and its descendants.
    fn node_opacity(&self, id: NodeId) -> f32 {
        let _ = id;
        1.0
    }
    fn node_children(&self, id: NodeId) -> &[NodeId];
    fn node_picture_policy(&self, id: NodeId) -> PicturePolicy {
        let _ = id;
        PicturePolicy::Never
    }
    fn node_has_semantic_handlers(&self, id: NodeId) -> bool {
        let _ = id;
        false
    }
    fn node_has_dynamic_content(&self, id: NodeId) -> bool {
        let _ = id;
        false
    }
    fn node_has_interactive_state(&self, id: NodeId) -> bool {
        let _ = id;
        false
    }
    fn node_wants_continuous_pointer_move(&self, id: NodeId) -> bool {
        let _ = id;
        false
    }
    fn node_is_overlay(&self, id: NodeId) -> bool {
        let _ = id;
        false
    }
    fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect>;
    fn dirty_rect(&self, id: NodeId, frame: Rect) -> Rect;
    /// 滚动容器内容偏移（viewport → content），无滚动时返回 `None`。
    fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)>;
    fn focused_node(&self) -> Option<NodeId>;
    fn node_focusable(&self, id: NodeId) -> bool;
    fn hit_test(&self, pos: Point) -> Option<NodeId>;
    fn parent(&self, id: NodeId) -> Option<NodeId>;
    fn paint(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext<'_>);
}
