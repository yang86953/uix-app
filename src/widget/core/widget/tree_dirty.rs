use super::tree_core::WidgetTree;
use super::*;
use crate::render::pipeline::{Invalidation, InvalidationQueueHandle};
use crate::render::DirtyRegion;

impl WidgetTree {
    pub fn invalidation(&self) -> &InvalidationQueueHandle {
        &self.invalidation
    }

    pub fn invalidation_handle(&self) -> InvalidationQueueHandle {
        self.invalidation.clone()
    }

    /// 是否有待渲染工作（Paint / Composite 失效，不含纯 Layout）。
    ///
    /// Layout 失效由 `layout()` 消费；若仅用 `is_empty()` 判定，
    /// 会在 dirty_region 为空时仍进入 render，导致 begin_frame 返回 Idle、画面不更新。
    pub fn has_render_work(&self) -> bool {
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .has_paint_or_composite()
    }

    /// 是否有进行中的动画/滚动惯性（仅用于事件轮询，不触发 present）。
    pub fn animations_active(&self) -> bool {
        self.animation_registry.has_active()
    }

    /// 绑定失效队列：初始化并扫描常驻动画节点。
    pub fn bind_invalidation(&mut self) {
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.animation_registry.clear();
        // 绑定后标记根节点 Layout 失效，确保 event_loop 首帧会执行 layout()
        // （否则 bind 清空队列后 layout_traverse 为空，子树 frame 无法初始化）
        if let Some(root_id) = self.root_id {
            self.push_layout_invalidation(root_id);
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
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Invalidation::Paint { id, rect });
    }

    pub(crate) fn push_layout_invalidation(&mut self, id: WidgetId) {
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Invalidation::Layout(id));
    }

    pub fn dirty_region(&self) -> DirtyRegion {
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .dirty_region()
    }

    /// 向上传播 Layout 失效到所有祖先。
    pub(crate) fn propagate_layout_invalidation(&mut self, from: WidgetId) {
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
            self.push_layout_invalidation(pid);
        }
    }

    /// 标记节点 Paint 失效（精确 dirty_rect）。
    pub fn invalidate_paint(&mut self, id: WidgetId) {
        if self.get(id).is_none() {
            return;
        }
        let rect = self.get(id).map(|node| {
            let frame = node.frame();
            let dirty = node.dirty_rect(frame);
            if dirty.w > 0.0 && dirty.h > 0.0 {
                dirty
            } else {
                frame
            }
        });
        if let Some(r) = rect.filter(|r| r.w > 0.0 && r.h > 0.0) {
            self.push_paint_invalidation(id, Some(r));
        }
        self.try_register_animation(id);
    }

    /// 标记指定矩形 Paint 失效。
    pub fn invalidate_paint_rect(&mut self, id: WidgetId, rect: Rect) {
        if self.get(id).is_none() {
            return;
        }
        if rect.w > 0.0 && rect.h > 0.0 {
            self.push_paint_invalidation(id, Some(rect));
        }
        self.try_register_animation(id);
    }

    /// 兼容旧 API。
    pub fn mark_dirty(&mut self, id: WidgetId) {
        self.invalidate_paint(id);
    }

    pub fn mark_dirty_rect(&mut self, id: WidgetId, rect: Rect) {
        self.invalidate_paint_rect(id, rect);
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
        for nid in ids {
            self.invalidate_paint(nid);
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

    /// 重置脏状态（帧末调用）。
    pub fn reset_dirty(&mut self) {
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.scroll_region_move = None;
    }

    /// 获取滚动偏移（用于 scroll_region 像素移动）。
    pub fn drain_scroll_region_move(&mut self) -> Option<(Rect, f32, f32)> {
        self.scroll_region_move.take()
    }

    pub fn mark_full_frame_dirty(&mut self) {
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Invalidation::Paint {
                id: 0,
                rect: None,
            });
        if let Some(root) = self.root_id {
            self.push_layout_invalidation(root);
        }
    }

    /// 绑定响应式 widget（DynamicLabel 等）的 State → Paint 失效。
    pub fn bind_reactive_widget_states(&mut self) {
        use crate::view::combinators::DynamicLabel;
        let handle = self.invalidation_handle();
        for id in self.traverse() {
            let type_id = self
                .get(id)
                .map(|n| n.component().as_any().type_id())
                .unwrap_or(std::any::TypeId::of::<()>());
            if type_id == std::any::TypeId::of::<DynamicLabel>() {
                let paint_rect = self.get(id).and_then(|n| {
                    let frame = n.frame();
                    let dirty = n.dirty_rect(frame);
                    let r = if dirty.w > 0.0 && dirty.h > 0.0 {
                        dirty
                    } else {
                        frame
                    };
                    if r.w > 0.0 && r.h > 0.0 {
                        Some(r)
                    } else {
                        None
                    }
                });
                if let Some(node) = self.get(id) {
                    if let Some(dl) = node.component().as_any().downcast_ref::<DynamicLabel>() {
                        dl.bind_state_invalidation(id, handle.clone(), paint_rect);
                    }
                }
            }
        }
    }
}
