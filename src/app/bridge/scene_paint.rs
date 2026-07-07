//! WidgetTree 的 ScenePaint 实现 — UI 与 draw compositor 的桥接。

use crate::core::DirtyRegion;
use crate::core::{Point, Rect};
use crate::draw::compositor::PicturePolicy;
use crate::draw::compositor::ScenePaint;
use crate::draw::painting::PaintContext;
use crate::draw::pipeline::NodeId;
use crate::ui::core::paint_scope::set_current_paint_widget;
use crate::ui::{WidgetCore, WidgetId, WidgetTree};

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

    fn node_picture_policy(&self, id: NodeId) -> PicturePolicy {
        self.get(id)
            .map(|n| n.picture_policy())
            .unwrap_or(PicturePolicy::Never)
    }

    fn node_has_semantic_handlers(&self, id: NodeId) -> bool {
        self.get(id)
            .is_some_and(|n| !n.handler_signatures().is_empty())
    }

    fn node_has_dynamic_content(&self, id: NodeId) -> bool {
        self.get(id).is_some_and(|n| n.has_dynamic_content())
    }

    fn node_has_interactive_state(&self, id: NodeId) -> bool {
        self.get(id).is_some_and(|n| n.has_interactive_state())
    }

    fn node_wants_continuous_pointer_move(&self, id: NodeId) -> bool {
        self.get(id)
            .is_some_and(|n| n.wants_continuous_pointer_move())
    }

    fn node_is_overlay(&self, id: NodeId) -> bool {
        self.get(id)
            .is_some_and(|n| n.overlay_entry(id, n.frame()).is_some())
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
        self.managers().focus.focused_widget()
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
                set_current_paint_widget(Some(id));
                node.render(frame, ctx, self);
                set_current_paint_widget(None);
            }
        }
    }
}

// WidgetId 与 NodeId 同型
const _: () = assert!(std::mem::size_of::<WidgetId>() == std::mem::size_of::<NodeId>());
