//! ScenePaint — 场景绘制抽象，解耦 compositor 与 uix-ui。

use uix_platform::{Point, Rect};

use crate::painting::PaintContext;
use crate::pipeline::NodeId;
use crate::types::DirtyRegion;

/// 场景绘制契约：LayerTree 通过此 trait 读取节点元数据并下发绘制。
pub trait ScenePaint {
    fn root_id(&self) -> Option<NodeId>;
    fn tree_version(&self) -> u64;
    fn dirty_region(&self) -> DirtyRegion;
    fn node_visible(&self, id: NodeId) -> bool;
    fn node_frame(&self, id: NodeId) -> Rect;
    fn node_dirty(&self, id: NodeId) -> bool;
    fn node_z_index(&self, id: NodeId) -> i32;
    fn node_children(&self, id: NodeId) -> &[NodeId];
    fn is_repaint_boundary(&self, id: NodeId) -> bool;
    fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect>;
    fn dirty_rect(&self, id: NodeId, frame: Rect) -> Rect;
    /// 滚动容器内容偏移（viewport → content），无滚动时返回 `None`。
    fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)>;
    fn focused_node(&self) -> Option<NodeId>;
    fn node_focusable(&self, id: NodeId) -> bool;
    fn hit_test(&self, pos: Point) -> Option<NodeId>;
    fn parent(&self, id: NodeId) -> Option<NodeId>;
    fn paint(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext<'_>);
    fn paint_overlay(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext<'_>);
}
