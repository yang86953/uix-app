use super::tree_core::WidgetTree;
use super::*;
use crate::core::DirtyRegion;
use crate::draw::renderer::{Invalidation, InvalidationQueueHandle, ScrollDelta};
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[cfg(test)]
#[path = "../../../../tests/unit/ui/widget_runtime/widget/tree_dirty__tests.rs"]
mod tests;

// 返回组件外框中没有被内容搬移视口覆盖的非重叠区域。
fn scroll_chrome_regions(frame: Rect, viewport: Rect) -> Vec<Rect> {
    // 调用方通常已做相交；这里仍防御组件返回越界视口。
    let Some(viewport) = frame.intersect(&viewport) else {
        // 完全不相交时整个组件都属于不能搬移的绘制区域。
        return (frame.w > 0.0 && frame.h > 0.0)
            .then_some(frame)
            .into_iter()
            .collect();
    };
    // 最多产生上、下、左、右四个互不重叠的矩形。
    let mut regions = Vec::with_capacity(4);
    let frame_right = frame.x + frame.w;
    let frame_bottom = frame.y + frame.h;
    let viewport_right = viewport.x + viewport.w;
    let viewport_bottom = viewport.y + viewport.h;
    for region in [
        Rect::new(frame.x, frame.y, frame.w, viewport.y - frame.y),
        Rect::new(
            frame.x,
            viewport_bottom,
            frame.w,
            frame_bottom - viewport_bottom,
        ),
        Rect::new(frame.x, viewport.y, viewport.x - frame.x, viewport.h),
        Rect::new(
            viewport_right,
            viewport.y,
            frame_right - viewport_right,
            viewport.h,
        ),
    ] {
        // 空边不进入失效队列。
        if region.w > 0.0 && region.h > 0.0 {
            regions.push(region);
        }
    }
    regions
}

impl WidgetTree {
    /// 返回组件树共享的失效队列句柄。
    pub fn invalidation(&self) -> &InvalidationQueueHandle {
        &self.invalidation
    }

    /// 克隆组件树共享的失效队列句柄，供树外调度器持有。
    pub fn invalidation_handle(&self) -> InvalidationQueueHandle {
        self.invalidation.clone()
    }

    pub(crate) fn invalidation_revision(&self) -> u64 {
        self.invalidation
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .revision()
    }

    /// 返回一个线程安全回调，用于请求所属组件树执行声明协调。
    pub fn reconcile_requester(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::clone(&self.reconcile_callback)
    }

    /// 返回协调请求端口在当前组件树中的稳定身份键。
    pub fn reconcile_requester_key(&self) -> usize {
        Arc::as_ptr(&self.reconcile_requested) as usize
    }

    /// 原子消费一次待处理协调请求；非运行态组件树始终返回 `false`。
    pub fn take_reconcile_requested(&self) -> bool {
        // 非运行态树不得让窗口驱动在协调中或失败后重入声明更新。
        if !self.accepts_external_work() {
            // 永久停止状态直接消费旧请求，避免 teardown 前形成空转信号。
            if self.is_fail_stopped() {
                // 清除失败前已发布的请求位。
                self.reconcile_requested.store(false, Ordering::Release);
            }
            // 协调中的请求保留到事务成功后，当前调用只报告无工作。
            return false;
        }
        self.reconcile_requested.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn has_reconcile_requested(&self) -> bool {
        // 非运行态不得让调度器因半树请求保持活跃。
        if !self.accepts_external_work() {
            // 协调成功后原子位仍可在 Operational 阶段重新观察。
            return false;
        }
        self.reconcile_requested.load(Ordering::Acquire)
    }

    /// 是否有待渲染工作（Paint / Composite 失效，不含纯 Layout）。
    ///
    /// Layout 失效由 `layout()` 消费；若仅用 `is_empty()` 判定，
    /// 会在 dirty_region 为空时仍进入 render，导致 begin_frame 返回 Idle、画面不更新。
    pub fn has_render_work(&self) -> bool {
        // 已停止的树不得继续请求渲染半提交的节点结构。
        if !self.accepts_external_work() {
            // 对调度器报告没有可安全渲染的工作。
            return false;
        }
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
        // retained texture copy 是整数像素事务；分数位移若取整会让文字与新绘像素逐帧漂移。
        let integral_geometry = [viewport.x, viewport.y, viewport.w, viewport.h, dx, dy]
            .into_iter()
            .all(|value| value.is_finite() && value.fract() == 0.0);
        if !integral_geometry || viewport.w <= 0.0 || viewport.h <= 0.0 || (dx == 0.0 && dy == 0.0)
        {
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

    // 为组件登记一次精确内容搬移，并重绘搬移视口外的滚动条或固定 chrome。
    pub(crate) fn push_node_scroll_composite(&mut self, id: WidgetId, dx: f32, dy: f32) -> bool {
        let Some((frame, logical_viewport)) = self.get(id).map(|node| {
            let frame = node.frame();
            (frame, node.scroll_composite_viewport(frame))
        }) else {
            return false;
        };
        // 纹理搬移消费屏幕坐标，必须与绘制、命中共用祖先滚动和裁剪投影。
        let Some(visual_viewport) = self.clipped_visual_rect(id, logical_viewport) else {
            return false;
        };
        if !self.push_scroll_composite(visual_viewport, dx, dy) {
            return false;
        }
        // 内容视口之外不能复用旧像素；滚动条滑块位置也会随偏移变化。
        for region in scroll_chrome_regions(frame, logical_viewport) {
            if let Some(region) = self.clipped_visual_rect(id, region) {
                self.push_paint_invalidation(id, Some(region));
            }
        }
        true
    }

    /// 返回当前失效队列聚合得到的绘制脏区快照。
    pub fn dirty_region(&self) -> DirtyRegion {
        self.invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .dirty_region()
    }

    /// 向上传播 Layout 失效到所有祖先。
    pub(crate) fn propagate_layout_invalidation(&mut self, from: WidgetId) {
        let mut chain = std::mem::take(&mut self.layout_ancestor_scratch);
        chain.clear();
        let mut current = self.get(from).and_then(|n| n.parent());
        while let Some(pid) = current {
            chain.push(pid);
            current = self.get(pid).and_then(|n| n.parent());
        }
        self.push_invalidations(chain.iter().copied().map(Invalidation::Layout));
        chain.clear();
        self.layout_ancestor_scratch = chain;
    }

    /// 标记节点 Paint 失效（精确 dirty_rect）。
    pub fn invalidate_paint(&mut self, id: WidgetId) {
        if self.get(id).is_none() {
            return;
        }
        let transformed = self.path_has_visual_transform(id);
        let scroll = self.get(id).and_then(|node| node.scroll_delta_for_dirty());
        if !transformed {
            if let Some((dx, dy)) = scroll {
                if self.push_node_scroll_composite(id, dx, dy) {
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
            .and_then(|rect| self.clipped_visual_rect(id, rect))
        {
            self.push_paint_invalidation(id, Some(r));
        }
    }

    /// 标记指定矩形 Paint 失效。
    pub fn invalidate_paint_rect(&mut self, id: WidgetId, rect: Rect) {
        if self.get(id).is_none() {
            return;
        }
        if let Some(rect) = (rect.w > 0.0 && rect.h > 0.0)
            .then(|| self.clipped_visual_rect(id, rect))
            .flatten()
        {
            self.push_paint_invalidation(id, Some(rect));
        }
    }

    /// 将指定节点及其全部后代批量标记为绘制失效。
    pub fn invalidate_paint_subtree(&mut self, id: WidgetId) {
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

    /// Clears the frame's consumed work only if painting/presentation did not
    /// enqueue more work after the caller sampled `revision`.
    pub(crate) fn reset_invalidation_if_revision(&mut self, revision: u64) -> bool {
        if !self.pending_invalidations.is_empty() {
            return false;
        }
        let cleared = self
            .invalidation
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear_if_revision(revision);
        if cleared {
            self.scroll_region_moves.clear();
        }
        cleared
    }

    /// 获取同帧全部滚动视口；提交成功前保留，供失败帧原样重试。
    pub(crate) fn scroll_region_moves(&self) -> Option<Vec<(Rect, f32, f32)>> {
        if self.scroll_region_moves.is_empty() {
            None
        } else {
            Some(self.scroll_region_moves.clone())
        }
    }

    /// 将根节点同时标记为完整绘制与布局失效。
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

    /// Forces one full compositor pass without claiming that the ordinary
    /// widget tree changed. Overlay membership has already been resolved by
    /// layout; only the root-level composition topology is new.
    pub(crate) fn mark_full_frame_composite(&mut self) {
        self.push_invalidation(Invalidation::FullComposite);
    }

    /// 绑定响应式 widget（DynamicLabel 等）的 State → Paint 失效。
    pub fn bind_reactive_widget_states(&mut self) {
        use crate::ui::reactive::state::StateBindCaptureGuard;
        use crate::ui::widget_runtime::dynamic_label::DynamicLabel;
        let handle = self.invalidation_handle();
        for &id in self.traverse().iter() {
            let type_id = self
                .get(id)
                .map(|n| n.widget().as_any().type_id())
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
                        self.clipped_visual_rect(id, r)
                    } else {
                        None
                    }
                });
                if let Some(node) = self.get(id) {
                    if let Some(dl) = node.widget().as_any().downcast_ref::<DynamicLabel>() {
                        // 以可在 panic 时自动恢复的作用域探测闭包依赖。
                        let capture = StateBindCaptureGuard::begin(id, handle.clone(), paint_rect);
                        // 探测闭包运行时读取的 State（含 View 外创建的实例，如 README Counter）。
                        dl.probe_dependencies();
                        // 正常完成后把精确绘制订阅交给实际节点。
                        let leases = capture.finish();
                        // 本轮集合整体替换旧依赖，防止重布局累积陈旧站点。
                        node.replace_paint_state_binds(leases);
                    }
                }
            }
        }
    }

    /// 将捕获根显式交接的结构性 State 绑定为 reconcile。
    ///
    /// 构建期 `State::get()` 表示 View 结构依赖该值（如 demo 的 `active` 选页）。
    /// 若只绑根节点 Paint，变更会全帧重绘却不重建树——页面不切换，且
    /// `layer_tree.render` 全帧 record 可达数百毫秒。
    /// DynamicLabel 等文本闭包依赖由 `bind_reactive_widget_states` 单独绑 Paint。
    pub(crate) fn replace_root_captured_state_binds(
        &mut self,
        state_binds: Vec<std::sync::Arc<dyn crate::ui::reactive::state::StatePaintBind>>,
    ) {
        // 停止树不得重新订阅任何声明 State 或持有新的租约。
        assert!(self.accepts_coordination_work());
        // 读取所属树的窄 reconcile 请求端口。
        let reconcile = self.reconcile_requester();
        // 读取所属树的稳定订阅去重键。
        let reconcile_key = self.reconcile_requester_key();
        // 为根本轮全部 State 源建立由根生命周期持有的租约。
        let leases = state_binds
            // 逐个转换声明源为可自动解绑的租约。
            .into_iter()
            // 将当前树请求端口安装到每个源。
            .map(|source| {
                // 返回由根集合拥有的精确订阅租约。
                crate::ui::reactive::state::ReconcileBindLease::bind(
                    // 转移当前声明 State 源。
                    source,
                    // 传入稳定树请求端口键。
                    reconcile_key,
                    // 共享当前树的请求端口。
                    reconcile.clone(),
                )
            })
            // 收集本轮完整根租约集合。
            .collect();
        // 替换根集合并让旧根租约在最后持有者离开时解绑。
        self.root_reconcile_state_binds = leases;
    }

    // 整体替换一个实际节点持有的结构性 State 租约。
    pub(crate) fn replace_node_captured_state_binds(
        // 独占访问节点和所属树的请求端口。
        &mut self,
        // 接收实际已挂载节点标识。
        id: WidgetId,
        // 接收该节点本轮声明捕获的 State 源。
        state_binds: Vec<std::sync::Arc<dyn crate::ui::reactive::state::StatePaintBind>>,
    ) {
        // 读取所属树的窄 reconcile 请求端口。
        let reconcile = self.reconcile_requester();
        // 读取所属树的稳定订阅去重键。
        let reconcile_key = self.reconcile_requester_key();
        // 为节点本轮 State 源建立可自动解绑租约。
        let leases = state_binds
            // 逐个转换声明源为节点生命周期租约。
            .into_iter()
            // 将当前树请求端口安装到每个源。
            .map(|source| {
                // 返回由节点集合拥有的精确订阅租约。
                crate::ui::reactive::state::ReconcileBindLease::bind(
                    // 转移当前声明 State 源。
                    source,
                    // 传入稳定树请求端口键。
                    reconcile_key,
                    // 共享当前树的请求端口。
                    reconcile.clone(),
                )
            })
            // 收集该节点完整租约集合。
            .collect();
        // 仅实际节点仍存在时替换其租约，缺席时 leases 立即回滚解绑。
        if let Some(node) = self.get_mut(id) {
            // 让节点拥有本轮租约。
            node.replace_reconcile_state_binds(leases);
        }
    }

    /// 注册捕获根显式交接的 Effect。
    /// 每次 rebuild 创建新的 Effect 实例，先清除旧实例避免累积。
    pub(crate) fn register_root_effects(
        &mut self,
        effects: Vec<crate::ui::reactive::state::Effect>,
    ) {
        // 停止树不得重新接管会保留应用资源的 Effect。
        assert!(self.accepts_coordination_work());
        // 根替换前先释放旧根 Effect，保持 rebuild 的替换语义。
        self.effects.clear();
        // 接管当前根显式交接的全部 Effect 实例。
        self.effects.extend(effects);
    }

    /// 返回运行态组件树是否拥有等待执行的根级或节点级 Effect。
    pub fn has_pending_effects(&self) -> bool {
        // 已停止的树不得再把 Effect 作为待执行工作暴露给窗口循环。
        if !self.accepts_external_work() {
            // 对调度器报告没有可安全 tick 的 Effect。
            return false;
        }
        // 先检查根捕获交接给树拥有的 Effect。
        self.effects.iter().any(|eff| eff.has_pending())
            // 再检查全部实际挂载节点生命周期拥有的 Effect。
            || self.traverse().iter().any(|&id| {
                // 节点可能在遍历期间因代际失效而不可用。
                self.get(id)
                    .is_some_and(|node| node.effects().iter().any(|eff| eff.has_pending()))
            })
    }

    /// 每帧 tick 已注册的 Effect；任一 Effect 重新执行时返回 true。
    /// 完整遍历所有 Effect，不短路，确保同一事件轮次全部执行。
    pub fn tick_effects(&self) -> bool {
        // 已停止的树不得执行 Effect 的用户闭包。
        if !self.accepts_external_work() {
            // 对调度器报告本帧没有 Effect 变化。
            return false;
        }
        let mut any_changed = false;
        for eff in &self.effects {
            // 执行下一根 Effect 前复核树是否仍接受外部工作。
            if !self.accepts_external_work() {
                // 前序 Effect 已使树停止时不再运行同级闭包。
                break;
            }
            if eff.tick() {
                any_changed = true;
            }
            // 根 Effect 返回后立即确认其没有使树进入停止态。
            if !self.accepts_external_work() {
                // 停止态不得继续进入剩余根或节点 Effect。
                break;
            }
        }
        // 逐个节点处理其私有 Effect，并在停止态中止整个节点遍历。
        'node_effects: for &id in self.traverse().iter() {
            // 读取下一节点前复核前序 Effect 没有停止整棵树。
            if !self.accepts_external_work() {
                // 首个停止态会阻止所有后续节点 Effect。
                break;
            }
            // 真实移除前的 leave 节点仍保留其 Effect 并继续参与调度。
            if let Some(node) = self.get(id) {
                // 完整遍历当前节点拥有的所有 Effect。
                for effect in node.effects() {
                    // 执行本节点下一 Effect 前复核树的外部工作准入。
                    if !self.accepts_external_work() {
                        // 停止态必须跳出节点与同级节点的全部 Effect。
                        break 'node_effects;
                    }
                    // 合并本轮是否有任意 Effect 实际重新执行。
                    if effect.tick() {
                        // 记录至少一个节点 Effect 已更新。
                        any_changed = true;
                    }
                    // Effect 返回后立即阻止停止树继续执行同级闭包。
                    if !self.accepts_external_work() {
                        // 首个停止态必须跳出节点与同级节点的全部 Effect。
                        break 'node_effects;
                    }
                }
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
