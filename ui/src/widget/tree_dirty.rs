use super::tree_core::WidgetTree;
use super::*;
use uix_graphics::DirtyRegion;

impl WidgetTree {
    pub fn dirty_region(&self) -> &DirtyRegion { &self.dirty_region }

    pub fn mark_dirty(&mut self, id: WidgetId) {
        let is_valid = {
            if let Some(node) = self.get_mut(id) { node.set_dirty(true); true }
            else { false }
        };
        if !is_valid { return; }
        let (frame, dirty) = self.get(id).map(|node| {
            (node.frame(), node.inner().dirty_rect(node.frame()))
        }).unwrap_or_default();
        if dirty.w > 0.0 && dirty.h > 0.0 { self.dirty_region.add_rect(dirty); }
        else if frame.w > 0.0 && frame.h > 0.0 { self.dirty_region.add_rect(frame); }
    }

    pub fn mark_dirty_rect(&mut self, id: WidgetId, rect: Rect) {
        if let Some(node) = self.get_mut(id) { node.set_dirty(true); }
        if rect.w > 0.0 && rect.h > 0.0 { self.dirty_region.add_rect(rect); }
    }

    pub fn mark_dirty_subtree(&mut self, id: WidgetId) {
        let ids: Vec<WidgetId> = {
            let mut result = vec![id];
            if let Some(node) = self.get(id) {
                for &child_id in node.children() { self.collect_subtree(child_id, &mut result); }
            }
            result
        };
        for id in ids { self.mark_dirty(id); }
    }

    fn collect_subtree(&self, id: WidgetId, result: &mut Vec<WidgetId>) {
        result.push(id);
        if let Some(node) = self.get(id) {
            for &child_id in node.children() { self.collect_subtree(child_id, result); }
        }
    }

    pub fn drain_scroll_deltas(&mut self) -> Vec<(Rect, f32, f32)> {
        std::mem::take(&mut self.scroll_deltas)
    }

    pub fn reset_dirty(&mut self) {
        self.dirty_region.reset();
        let ids: Vec<WidgetId> = self.traverse();
        for id in ids { if let Some(node) = self.get_mut(id) { node.set_dirty(false); } }
    }

    pub fn mark_full_frame_dirty(&mut self) {
        self.dirty_region = DirtyRegion::full();
        for id in self.traverse() {
            if let Some(node) = self.get_mut(id) { node.set_dirty(true); }
        }
    }
}
