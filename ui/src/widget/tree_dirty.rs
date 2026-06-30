use super::tree_core::WidgetTree;
use super::*;
use uix_graphics::DirtyRegion;

impl WidgetTree {
    pub fn dirty_region(&self) -> &DirtyRegion { &self.dirty.region }

    /// 终极方案：从指定节点向上传播子树脏标记到所有祖先。
    pub(crate) fn propagate_subtree_dirty(&mut self, from: WidgetId) {
        let parents: Vec<WidgetId> = {
            let mut chain = Vec::new();
            let mut current = self.get(from).and_then(|n| n.parent());
            while let Some(pid) = current {
                chain.push(pid);
                current = self.get(pid).and_then(|n| n.parent());
            }
            chain
        };
        for pid in parents {
            self.subtree_dirty.insert(pid);
        }
    }

    pub fn mark_dirty(&mut self, id: WidgetId) {
        let is_valid = {
            if let Some(node) = self.get_mut(id) {
                node.set_dirty(true);
                self.dirty_nodes.insert(id);
                true
            } else { false }
        };
        if !is_valid { return; }
        let (frame, dirty) = self.get(id).map(|node| {
            (node.frame(), node.dirty_rect(node.frame()))
        }).unwrap_or_default();
        if dirty.w > 0.0 && dirty.h > 0.0 { self.dirty.region.add_rect(dirty); }
        else if frame.w > 0.0 && frame.h > 0.0 { self.dirty.region.add_rect(frame); }

        // 终极方案：向上传播子树脏标记
        self.propagate_subtree_dirty(id);
    }

    pub fn mark_dirty_rect(&mut self, id: WidgetId, rect: Rect) {
        if let Some(node) = self.get_mut(id) {
            node.set_dirty(true);
            self.dirty_nodes.insert(id);
        }
        if rect.w > 0.0 && rect.h > 0.0 { self.dirty.region.add_rect(rect); }

        // 终极方案：向上传播子树脏标记
        self.propagate_subtree_dirty(id);
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
        std::mem::take(&mut self.dirty.scroll_deltas)
    }

    /// 重置脏状态。只遍历脏节点清除 is_dirty，而非全量 traverse。
    /// 遍历复杂度与脏节点数成正比，与总节点数无关。
    pub fn reset_dirty(&mut self) {
        self.dirty.reset();
        let ids: Vec<WidgetId> = self.dirty_nodes.iter_dirty().collect();
        for id in ids {
            if let Some(node) = self.get_mut(id) {
                node.set_dirty(false);
            }
        }
        self.dirty_nodes.clear();
        self.subtree_dirty.clear();
    }

    pub fn mark_full_frame_dirty(&mut self) {
        self.dirty.region = DirtyRegion::full();
        for id in self.traverse() {
            self.dirty_nodes.insert(id);
            self.subtree_dirty.insert(id);
            if let Some(node) = self.get_mut(id) { node.set_dirty(true); }
        }
    }
}
