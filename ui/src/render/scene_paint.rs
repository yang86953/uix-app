//! WidgetTree 的 ScenePaint 实现 — UI 与 graphics compositor 的桥接。

use uix_graphics::compositor::ScenePaint;
use uix_graphics::painting::PaintContext;
use uix_graphics::pipeline::NodeId;
use uix_graphics::types::DirtyRegion;
use uix_platform::{Point, Rect};

use crate::widget::{WidgetCore, WidgetId, WidgetTree};

impl ScenePaint for WidgetTree {
    fn root_id(&self) -> Option<NodeId> {
        self.root_id()
    }

    fn tree_version(&self) -> u64 {
        self.tree_version()
    }

    fn dirty_region(&self) -> DirtyRegion {
        self.dirty_region()
    }

    fn node_visible(&self, id: NodeId) -> bool {
        self.get(id).is_some_and(|n| n.visible())
    }

    fn node_frame(&self, id: NodeId) -> Rect {
        self.get(id).map(|n| n.frame()).unwrap_or_default()
    }

    fn node_dirty(&self, id: NodeId) -> bool {
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .node_needs_paint(id)
    }

    fn node_z_index(&self, id: NodeId) -> i32 {
        self.get(id).map(|n| n.z_index()).unwrap_or(0)
    }

    fn node_children(&self, id: NodeId) -> &[NodeId] {
        static EMPTY: &[NodeId] = &[];
        self.get(id).map(|n| n.children()).unwrap_or(EMPTY)
    }

    fn is_repaint_boundary(&self, id: NodeId) -> bool {
        self.get(id).is_some_and(|n| n.is_repaint_boundary())
    }

    fn children_clip(&self, id: NodeId, frame: Rect) -> Option<Rect> {
        self.get(id).and_then(|n| n.children_clip(frame))
    }

    fn dirty_rect(&self, id: NodeId, frame: Rect) -> Rect {
        self.get(id).map(|n| n.dirty_rect(frame)).unwrap_or(frame)
    }

    fn scroll_offset(&self, id: NodeId) -> Option<(f32, f32)> {
        self.get(id).and_then(|n| n.viewport_scroll_offset())
    }

    fn focused_node(&self) -> Option<NodeId> {
        self.focused_widget
    }

    fn node_focusable(&self, id: NodeId) -> bool {
        self.get(id).is_some_and(|n| n.is_focusable())
    }

    fn hit_test(&self, pos: Point) -> Option<NodeId> {
        self.hit_test(pos)
    }

    fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.parent())
    }

    fn paint(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext<'_>) {
        if let Some(node) = self.get(id) {
            if node.visible() {
                node.render(frame, ctx, self);
            }
        }
    }

    fn paint_overlay(&self, id: NodeId, frame: Rect, ctx: &mut PaintContext<'_>) {
        if let Some(node) = self.get(id) {
            if node.visible() {
                node.post_render(frame, ctx, self);
            }
        }
    }
}

// WidgetId 与 NodeId 同型
const _: () = assert!(std::mem::size_of::<WidgetId>() == std::mem::size_of::<NodeId>());
