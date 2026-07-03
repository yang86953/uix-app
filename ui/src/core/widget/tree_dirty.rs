use super::tree_core::WidgetTree;
use super::*;
use uix_graphics::pipeline::{Invalidation, InvalidationQueue, ScrollDelta};
use uix_graphics::DirtyRegion;

impl WidgetTree {
    pub fn invalidation(&self) -> &InvalidationQueue {
        &self.invalidation
    }

    /// 是否有待渲染工作（失效队列或脏区域非空）。
    pub fn has_render_work(&self) -> bool {
        !self.invalidation.is_empty() || !self.dirty.region.is_empty()
    }

    /// 是否有进行中的动画/滚动惯性（仅用于事件轮询，不触发 present）。
    pub fn animations_active(&self) -> bool {
        self.animation_registry.has_active()
    }

    /// 绑定失效队列：初始化并扫描常驻动画节点。
    pub fn bind_invalidation(&mut self) {
        self.invalidation.clear();
        self.animation_registry.clear();
        if self.root_id.is_some() {
            for id in self.traverse() {
                self.try_register_animation(id);
            }
        }
    }

    pub(crate) fn try_register_animation(&mut self, id: WidgetId) {
        if self
            .get(id)
            .is_some_and(|n| n.visible() && n.needs_continuous_update())
        {
            self.animation_registry.register(id);
        }
    }

    pub(crate) fn push_paint_invalidation(&mut self, id: WidgetId, rect: Option<Rect>) {
        self.invalidation.push(Invalidation::Paint { id, rect });
    }

    pub(crate) fn push_composite_invalidation(
        &mut self,
        rect: Rect,
        scroll: Option<ScrollDelta>,
    ) {
        self.invalidation
            .push(Invalidation::Composite { rect, scroll });
    }

    pub fn dirty_region(&self) -> &DirtyRegion {
        &self.dirty.region
    }

    /// 从指定节点向上传播脏标记到所有祖先。
    /// 既更新 `subtree_dirty`（布局遍历用），
    /// 也设置 `is_dirty` 标志并加入 `dirty_nodes`（渲染用）。
    /// 这样当事件命中子节点但由父节点处理时，父节点也会被正确重绘。
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
            // 同时设置 is_dirty 标志并加入 dirty_nodes，
            // 确保祖先节点在渲染时被正确识别为脏。
            if let Some(node) = self.get_mut(pid) {
                node.set_dirty(true);
            }
            self.dirty_nodes.insert(pid);
        }
    }

    pub fn mark_dirty(&mut self, id: WidgetId) {
        let is_valid = {
            if let Some(node) = self.get_mut(id) {
                node.set_dirty(true);
                self.dirty_nodes.insert(id);
                true
            } else {
                false
            }
        };
        if !is_valid {
            return;
        }
        let (frame, dirty) = self
            .get(id)
            .map(|node| (node.frame(), node.dirty_rect(node.frame())))
            .unwrap_or_default();
        if dirty.w > 0.0 && dirty.h > 0.0 {
            self.dirty.region.add_rect(dirty);
            self.push_paint_invalidation(id, Some(dirty));
        } else if frame.w > 0.0 && frame.h > 0.0 {
            self.dirty.region.add_rect(frame);
            self.push_paint_invalidation(id, Some(frame));
        }

        self.try_register_animation(id);

        // 终极方案：向上传播子树脏标记
        self.propagate_subtree_dirty(id);
    }

    pub fn mark_dirty_rect(&mut self, id: WidgetId, rect: Rect) {
        if let Some(node) = self.get_mut(id) {
            node.set_dirty(true);
            self.dirty_nodes.insert(id);
        }
        if rect.w > 0.0 && rect.h > 0.0 {
            self.dirty.region.add_rect(rect);
            self.push_paint_invalidation(id, Some(rect));
        }

        self.try_register_animation(id);

        // 终极方案：向上传播子树脏标记
        self.propagate_subtree_dirty(id);
    }

    pub fn mark_dirty_subtree(&mut self, id: WidgetId) {
        let ids: Vec<WidgetId> = {
            let mut result = vec![id];
            if let Some(node) = self.get(id) {
                for &child_id in node.children() {
                    self.collect_subtree(child_id, &mut result);
                }
            }
            result
        };
        for id in ids {
            self.mark_dirty(id);
        }
    }

    fn collect_subtree(&self, id: WidgetId, result: &mut Vec<WidgetId>) {
        result.push(id);
        if let Some(node) = self.get(id) {
            for &child_id in node.children() {
                self.collect_subtree(child_id, result);
            }
        }
    }

    /// 重置脏状态。只遍历脏节点清除 is_dirty，而非全量 traverse。
    /// 遍历复杂度与脏节点数成正比，与总节点数无关。
    pub fn reset_dirty(&mut self) {
        self.dirty.reset();
        self.invalidation.clear();
        let ids: Vec<WidgetId> = self.dirty_nodes.iter_dirty().collect();
        for id in ids {
            if let Some(node) = self.get_mut(id) {
                node.set_dirty(false);
            }
        }
        self.dirty_nodes.clear();
        self.subtree_dirty.clear();
    }

    /// 获取滚动偏移（用于 scroll_region 像素移动）。
    pub fn drain_scroll_region_move(&mut self) -> Option<(Rect, f32, f32)> {
        self.dirty.scroll_region_move.take()
    }

    pub fn mark_full_frame_dirty(&mut self) {
        self.dirty.region = DirtyRegion::full();
        self.invalidation.push(Invalidation::Paint { id: 0, rect: None });
        for id in self.traverse() {
            self.dirty_nodes.insert(id);
            self.subtree_dirty.insert(id);
            if let Some(node) = self.get_mut(id) {
                node.set_dirty(true);
            }
        }
    }
}
