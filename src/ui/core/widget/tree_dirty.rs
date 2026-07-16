use super::tree_core::WidgetTree;
use super::*;
use crate::core::DirtyRegion;
use crate::draw::pipeline::{Invalidation, InvalidationQueueHandle, ScrollDelta};
use std::sync::atomic::Ordering;
use std::sync::Arc;

impl WidgetTree {
    pub fn invalidation(&self) -> &InvalidationQueueHandle {
        &self.invalidation
    }

    pub fn invalidation_handle(&self) -> InvalidationQueueHandle {
        self.invalidation.clone()
    }

    pub fn reconcile_requester(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::clone(&self.reconcile_callback)
    }

    pub fn reconcile_requester_key(&self) -> usize {
        Arc::as_ptr(&self.reconcile_requested) as usize
    }

    pub fn take_reconcile_requested(&self) -> bool {
        self.reconcile_requested.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn has_reconcile_requested(&self) -> bool {
        self.reconcile_requested.load(Ordering::Acquire)
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

    /// 绑定失效队列。
    pub fn bind_invalidation(&mut self) {
        self.pending_invalidations.clear();
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        // 绑定后标记根节点 Layout 失效，确保 event_loop 首帧会执行 layout()
        // （否则 bind 清空队列后 layout_traverse 为空，子树 frame 无法初始化）
        if let Some(root_id) = self.root_id {
            self.push_layout_invalidation(root_id);
        }
    }

    pub(crate) fn push_paint_invalidation(&mut self, id: WidgetId, rect: Option<Rect>) {
        self.push_invalidation(Invalidation::Paint { id, rect });
    }

    pub(crate) fn push_layout_invalidation(&mut self, id: WidgetId) {
        self.push_invalidation(Invalidation::Layout(id));
    }

    pub(crate) fn begin_invalidation_batch(&mut self) {
        self.invalidation_batch_depth = self.invalidation_batch_depth.saturating_add(1);
    }

    pub(crate) fn finish_invalidation_batch(&mut self) {
        if self.invalidation_batch_depth == 0 {
            return;
        }
        self.invalidation_batch_depth -= 1;
        if self.invalidation_batch_depth > 0 || self.pending_invalidations.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_invalidations);
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(pending);
    }

    fn push_invalidation(&mut self, invalidation: Invalidation) {
        self.push_invalidations(std::iter::once(invalidation));
    }

    fn push_invalidations(&mut self, invalidations: impl IntoIterator<Item = Invalidation>) {
        if self.invalidation_batch_depth > 0 {
            self.pending_invalidations.extend(invalidations);
            return;
        }
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(invalidations);
    }

    pub(crate) fn push_scroll_composite(&mut self, viewport: Rect, dx: f32, dy: f32) -> bool {
        let dx = dx.round();
        let dy = dy.round();
        if viewport.w <= 0.0 || viewport.h <= 0.0 || (dx == 0.0 && dy == 0.0) {
            return false;
        }

        let mut exposed = None;
        if dx != 0.0 {
            let w = dx.abs().min(viewport.w);
            let x = if dx > 0.0 {
                viewport.x + viewport.w - w
            } else {
                viewport.x
            };
            exposed = Some(Rect::new(x, viewport.y, w, viewport.h));
        }
        if dy != 0.0 {
            let h = dy.abs().min(viewport.h);
            let y = if dy > 0.0 {
                viewport.y + viewport.h - h
            } else {
                viewport.y
            };
            let strip = Rect::new(viewport.x, y, viewport.w, h);
            exposed = Some(match exposed {
                Some(rect) => union_rect(rect, strip),
                None => strip,
            });
        }

        let Some(rect) = exposed.filter(|r| r.w > 0.0 && r.h > 0.0) else {
            return false;
        };

        self.push_invalidation(Invalidation::Composite {
            rect,
            scroll: Some(ScrollDelta { dx, dy }),
        });
        self.scroll_region_moves.push((viewport, dx, dy));
        true
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
        self.push_invalidations(parents.into_iter().map(Invalidation::Layout));
    }

    /// 标记节点 Paint 失效（精确 dirty_rect）。
    pub fn invalidate_paint(&mut self, id: ComponentId) {
        if self.get(id).is_none() {
            return;
        }
        let transformed = self.path_has_visual_transform(id);
        let scroll = self.get(id).and_then(|node| {
            let frame = node.frame();
            node.scroll_delta_for_dirty().map(|(dx, dy)| {
                let viewport = node.scroll_composite_viewport(frame);
                (viewport, dx, dy)
            })
        });
        if !transformed {
            if let Some((viewport, dx, dy)) = scroll {
                if self.push_scroll_composite(viewport, dx, dy) {
                    return;
                }
            }
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
        if let Some(r) = rect
            .filter(|r| r.w > 0.0 && r.h > 0.0)
            .and_then(|rect| self.node_visual_rect(id, rect))
        {
            self.push_paint_invalidation(id, Some(r));
        }
    }

    /// 标记指定矩形 Paint 失效。
    pub fn invalidate_paint_rect(&mut self, id: ComponentId, rect: Rect) {
        if self.get(id).is_none() {
            return;
        }
        if let Some(rect) = (rect.w > 0.0 && rect.h > 0.0)
            .then(|| self.node_visual_rect(id, rect))
            .flatten()
        {
            self.push_paint_invalidation(id, Some(rect));
        }
    }

    pub fn invalidate_paint_subtree(&mut self, id: ComponentId) {
        let ids: Vec<WidgetId> = {
            let mut result = vec![id];
            if let Some(node) = self.get(id) {
                for &child_id in node.children() {
                    self.collect_subtree(child_id, &mut result);
                }
            }
            result
        };
        self.begin_invalidation_batch();
        for nid in ids {
            self.invalidate_paint(nid);
        }
        self.finish_invalidation_batch();
    }

    fn collect_subtree(&self, id: WidgetId, result: &mut Vec<WidgetId>) {
        result.push(id);
        if let Some(node) = self.get(id) {
            for &child_id in node.children() {
                self.collect_subtree(child_id, result);
            }
        }
    }

    /// 清空失效队列（帧末调用）。
    pub fn reset_invalidation(&mut self) {
        self.pending_invalidations.clear();
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.scroll_region_moves.clear();
    }

    /// 获取同帧全部滚动视口；提交成功前保留，供失败帧原样重试。
    pub(crate) fn scroll_region_moves(&self) -> Option<Vec<(Rect, f32, f32)>> {
        if self.scroll_region_moves.is_empty() {
            None
        } else {
            Some(self.scroll_region_moves.clone())
        }
    }

    pub fn mark_full_frame_dirty(&mut self) {
        if let Some(root) = self.root_id {
            self.push_invalidations([
                Invalidation::Paint {
                    id: root,
                    rect: None,
                },
                Invalidation::Layout(root),
            ]);
        }
    }

    /// 绑定响应式 widget（DynamicLabel 等）的 State → Paint 失效。
    pub fn bind_reactive_widget_states(&mut self) {
        use crate::ui::foundation::state::{begin_state_bind_capture, end_state_bind_capture};
        use crate::ui::view::combinators::DynamicLabel;
        let handle = self.invalidation_handle();
        for &id in self.traverse().iter() {
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
                        self.node_visual_rect(id, r)
                    } else {
                        None
                    }
                });
                if let Some(node) = self.get(id) {
                    if let Some(dl) = node.component().as_any().downcast_ref::<DynamicLabel>() {
                        dl.bind_state_invalidation(id, handle.clone(), paint_rect);
                        // 探测闭包运行时读取的 State（含 View 外创建的实例，如 README Counter）
                        begin_state_bind_capture(id, handle.clone(), paint_rect);
                        dl.probe_dependencies();
                        end_state_bind_capture(id);
                    }
                }
            }
        }
    }

    /// 将 View 构建期未关联 widget 的 pending State 绑定为 reconcile。
    ///
    /// 构建期 `State::get()` 表示 View 结构依赖该值（如 demo 的 `active` 选页）。
    /// 若只绑根节点 Paint，变更会全帧重绘却不重建树——页面不切换，且
    /// `layer_tree.render` 全帧 record 可达数百毫秒。
    /// DynamicLabel 等文本闭包依赖由 `bind_reactive_widget_states` 单独绑 Paint。
    pub fn bind_orphan_pending_states(&mut self) {
        use crate::ui::foundation::state::drain_pending_state_binds;
        let orphans = drain_pending_state_binds();
        if orphans.is_empty() {
            return;
        }
        let reconcile = self.reconcile_requester();
        let reconcile_key = self.reconcile_requester_key();
        for source in orphans {
            source.bind_reconcile_site(reconcile_key, reconcile.clone());
        }
    }

    /// 注册 View 构建期捕获的 Effect。
    /// 每次 rebuild 创建新的 Effect 实例，先清除旧实例避免累积。
    pub fn bind_pending_effects(&mut self) {
        use crate::ui::foundation::state::drain_pending_effects;
        self.effects.clear();
        self.effects.extend(drain_pending_effects());
    }

    pub fn has_pending_effects(&self) -> bool {
        self.effects.iter().any(|eff| eff.has_pending())
    }

    /// 每帧 tick 已注册的 Effect；任一 Effect 重新执行时返回 true。
    /// 完整遍历所有 Effect，不短路，确保同一事件轮次全部执行。
    pub fn tick_effects(&self) -> bool {
        let mut any_changed = false;
        for eff in &self.effects {
            if eff.tick() {
                any_changed = true;
            }
        }
        any_changed
    }
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    let x1 = a.x.min(b.x);
    let y1 = a.y.min(b.y);
    let x2 = (a.x + a.w).max(b.x + b.w);
    let y2 = (a.y + a.h).max(b.y + b.h);
    Rect::new(x1, y1, x2 - x1, y2 - y1)
}
