use super::super::super::*;
use super::super::WidgetTree;
use crate::core::Rect;
use std::collections::HashSet;

#[cfg(test)]
use super::super::LAYOUT_TRACE_PHASE;

use super::{LayoutFrameScratch, LayoutTraversalScratch};

impl WidgetTree {
    pub(crate) fn nearest_viewport_overflow_axes(&self, id: WidgetId) -> Option<(bool, bool)> {
        crate::ui::tree_widget_hooks::nearest_viewport_overflow_axes(self, id)
    }

    /// Phase 2 不得撑开显式定宽/定高的节点（Container/Space/ScrollView 等；
    /// 组件语义见 System 私有边界 tree_widget_hooks）。
    pub(crate) fn phase2_explicit_size_locks(&self, id: WidgetId) -> (bool, bool) {
        crate::ui::tree_widget_hooks::phase2_explicit_size_locks(self, id)
    }

    /// 返回 Layout 失效影响的子树先序遍历顺序。
    ///
    /// 全帧或含 Layout 根时遍历对应子树；无 Layout 失效时返回空（跳过 layout）。
    pub fn layout_traverse(&self) -> Vec<WidgetId> {
        // 已停止的树不得向外暴露可能处于半提交状态的布局遍历。
        if !self.accepts_external_work() {
            // 没有可安全执行的布局节点。
            return Vec::new();
        }
        let mut result = Vec::new();
        self.fill_layout_traversal(&mut result, &mut LayoutTraversalScratch::default());
        result
    }

    pub(crate) fn fill_layout_traversal(
        &self,
        result: &mut Vec<WidgetId>,
        scratch: &mut LayoutTraversalScratch,
    ) {
        result.clear();
        scratch.paths.clear();
        scratch.stack.clear();
        let inv = self.invalidation.lock().unwrap_or_else(|e| e.into_inner());
        if inv.needs_full_frame() {
            result.extend(self.traverse().iter().copied());
            return;
        }
        inv.layout_roots_into(&mut scratch.roots);
        drop(inv);
        if scratch.roots.is_empty() {
            return;
        }

        for &id in &scratch.roots {
            let mut cur = Some(id);
            while let Some(cid) = cur {
                scratch.paths.insert(cid);
                cur = self.get(cid).and_then(|n| n.parent());
            }
        }

        if let Some(root_id) = self.root_id {
            scratch.stack.push((root_id, false));
            while let Some((current, parent_invalidated)) = scratch.stack.pop() {
                let invalidated = parent_invalidated || scratch.roots.contains(&current);
                if !invalidated && !scratch.paths.contains(&current) {
                    continue;
                }
                result.push(current);
                if let Some(node) = self.get(current) {
                    for &child_id in node.children().iter().rev() {
                        scratch.stack.push((child_id, invalidated));
                    }
                }
            }
        }
    }

    /// 对当前组件树执行一次完整布局，并发布相应生命周期变化。
    pub fn layout(&mut self) {
        // 已停止的树不得继续执行会调用组件代码的布局和生命周期协调。
        if !self.accepts_external_work() {
            // 保留故障现场直到所属窗口执行 teardown。
            return;
        }
        let has_valid_root = self
            .root_id
            .and_then(|id| self.get(id))
            .map(|r| r.frame().w > 0.0 && r.frame().h > 0.0)
            .unwrap_or(false);
        if !has_valid_root {
            if let Some(root_id) = self.root_id {
                // Bootstrap only: root may be created before a window/session assigns
                // a viewport-sized frame, so use natural size for the temporary frame.
                let ps = self
                    .get(root_id)
                    .map(|r| r.measure(Self::root_bootstrap_constraints()));
                if let Some(ps) = ps {
                    if let Some(root_mut) = self.get_mut(root_id) {
                        root_mut.set_frame(Rect::new(0.0, 0.0, ps.w.max(1.0), ps.h.max(1.0)));
                    }
                }
            }
        }

        let mut scratch = std::mem::take(&mut self.layout_scratch);
        self.layout_with_scratch(&mut scratch);
        self.layout_scratch = scratch;
    }

    pub(crate) fn layout_with_scratch(&mut self, scratch: &mut LayoutFrameScratch) {
        let LayoutFrameScratch {
            order,
            traversal,
            prev_expand_sig,
            pass_expand_sig,
            resized_children,
            visibility_changes,
            expand_children,
            shrink_ops,
            shrink_children,
            shrink_parent_children,
        } = scratch;

        self.refresh_collapse_content_children(order);
        self.refresh_image_error_children(order);
        // 表格 capability 启用时才在布局前刷新泛型单元格。
        #[cfg(feature = "table")]
        self.refresh_table_cell_children(order);
        self.fill_layout_traversal(order, traversal);
        if order.is_empty() {
            // 根节点已有有效 frame 但子树尚未布局时（如 bind_invalidation / reset 清空队列），
            // 只要仍有可见节点 frame 为 0 就重新标脏，避免组件堆叠在 (0,0)。
            // 不可用 measure().h<=0 作门槛：有 intrinsic 高度的子项也会卡在零 frame。
            let needs_bootstrap = self.root_id.is_some_and(|root_id| {
                self.get(root_id).is_some_and(|root| {
                    let rf = root.frame();
                    if rf.w <= 0.0 || rf.h <= 0.0 {
                        return true;
                    }
                    self.traverse().iter().copied().any(|cid| {
                        cid != root_id
                            && self.is_effectively_visible(cid)
                            && self
                                .get(cid)
                                .is_some_and(|c| c.frame().w <= 0.0 || c.frame().h <= 0.0)
                    })
                })
            });
            if !needs_bootstrap {
                return;
            }
            if let Some(root_id) = self.root_id {
                self.push_layout_invalidation(root_id);
            }
            self.fill_layout_traversal(order, traversal);
            if order.is_empty() {
                return;
            }
        }

        // ════════════════════════════════════════════════════════════════
        // 收敛循环：自上而下布局 → [扩展 ↔ 收缩] → viewport
        // Phase 2（扩展）和 Phase 4（收缩）交替运行直至稳定，
        // 防止两个阶段的尺寸调整形成逐帧振荡。
        // 上限提升至 10 次，应对深层嵌套（Container→Container→Widget）场景。
        // ════════════════════════════════════════════════════════════════
        let max_passes = 10;
        #[cfg(test)]
        let mut converge_passes = 0u32;
        // 安全网：若连续两轮 Phase 2 扩展签名完全相同（同 id、同 before/after），
        // 视为无 progress，停止空转（根因仍应在 measure；此处防止打满 max_passes）。
        prev_expand_sig.clear();
        pass_expand_sig.clear();
        let mut has_prev_expand_sig = false;
        resized_children.clear();
        visibility_changes.clear();
        for _converge_pass in 0..max_passes {
            #[cfg(test)]
            {
                converge_passes += 1;
            }
            let mut any_change = false;
            pass_expand_sig.clear();

            if self.sync_parent_child_visibility(order, visibility_changes) {
                any_change = true;
            }

            // Phase 1: Top-down — 父容器根据当前 frame 为子节点分配位置
            #[cfg(test)]
            LAYOUT_TRACE_PHASE.with(|p| p.set(1));
            for &id in order.iter() {
                if !self.is_effectively_visible(id) {
                    continue;
                }
                let positions: Vec<(WidgetId, Rect)> = {
                    let node = match self.get(id) {
                        Some(n) => n,
                        None => continue,
                    };
                    let frame = node.frame();
                    let children = node.children();
                    if children.is_empty() {
                        continue;
                    }
                    node.layout_children(frame, children, self)
                };
                for (child_id, rect) in positions {
                    if self.set_layout_frame(child_id, rect) {
                        any_change = true;
                    }
                }
            }
            if self.sync_parent_child_visibility(order, visibility_changes) {
                any_change = true;
            }
            #[cfg(test)]
            LAYOUT_TRACE_PHASE.with(|p| p.set(0));

            // 内循环：交替扩展和收缩直到稳定
            // 常规树维持较小预算；连续三轮仍未稳定才提高内循环预算，
            // 让深层动态子树加速向上传播，同时避免振荡页面在首轮做无效工作。
            let inner_passes = if _converge_pass < 3 { 3 } else { max_passes };
            let mut previous_inner_expand = None;
            for _inner_pass in 0..inner_passes {
                let expand_start = pass_expand_sig.len();
                #[cfg(test)]
                LAYOUT_TRACE_PHASE.with(|p| p.set(2));
                let expanded =
                    self.layout_expand(order, pass_expand_sig, resized_children, expand_children);
                let expand_end = pass_expand_sig.len();
                #[cfg(test)]
                LAYOUT_TRACE_PHASE.with(|p| p.set(4));
                let shrunk =
                    self.layout_shrink(order, shrink_ops, shrink_children, shrink_parent_children);
                #[cfg(test)]
                LAYOUT_TRACE_PHASE.with(|p| p.set(0));
                if expanded || shrunk {
                    any_change = true;
                }
                if !expanded && !shrunk {
                    break;
                }
                // 扩展与收缩若把同一批节点带回完全相同的尺寸，继续内循环只会空转。
                let repeated_expand =
                    previous_inner_expand.is_some_and(|(previous_start, previous_end)| {
                        expand_start < expand_end
                            && pass_expand_sig[previous_start..previous_end]
                                == pass_expand_sig[expand_start..expand_end]
                    });
                if repeated_expand {
                    break;
                }
                previous_inner_expand = Some((expand_start, expand_end));
            }

            // Phase 3: 更新 viewport 容器的 content_bounds
            self.layout_viewports(order);

            if !pass_expand_sig.is_empty() {
                if has_prev_expand_sig && prev_expand_sig == pass_expand_sig {
                    tracing::debug!(
                        "[Layout] Phase 2: identical expand signature — stop (no progress)",
                    );
                    break;
                }
                std::mem::swap(prev_expand_sig, pass_expand_sig);
                has_prev_expand_sig = true;
            }

            if !any_change {
                break;
            }
        }
        #[cfg(test)]
        {
            self.layout_converge_passes.set(converge_passes);
        }

        // 最终更新 viewport（确保收敛结束后的 content_bounds 正确）
        self.layout_viewports(order);
        self.refresh_virtual_scroll_children(order);
        // 表格 capability 启用时才用最终 viewport 再刷新泛型单元格。
        #[cfg(feature = "table")]
        self.refresh_table_cell_children(order);
        // Phase 6：layout 完成后用最新 frame 绑定 State → Paint rect
        self.bind_reactive_widget_states();
        self.rebuild_widget_overlays();
        self.reconcile_lifecycle_after_layout();
        self.sync_app_state_registry_from_lifecycle_states();
        tracing::debug!("[Layout] layout() done");
    }

    /// Synchronize parent-owned child visibility without overwriting a
    /// child's authored `visible` gate. A tree-version bump is required
    /// because the compositor omits invisible subtrees while building layers.
    pub(crate) fn sync_parent_child_visibility(
        &mut self,
        order: &[WidgetId],
        changes: &mut Vec<(WidgetId, bool)>,
    ) -> bool {
        // 保留同步前的焦点，待全部门控更新后按组件约定迁移。
        let focused_before = self.managers().focus.focused_widget();
        changes.clear();
        for &parent_id in order {
            let Some(parent) = self.get(parent_id) else {
                continue;
            };
            for (index, &child_id) in parent.children().iter().enumerate() {
                let desired = parent.child_visible(index);
                if self
                    .get(child_id)
                    .is_some_and(|child| child.parent_visibility_gate() != desired)
                {
                    changes.push((child_id, desired));
                }
            }
        }

        for (child_id, visible) in changes.iter() {
            let old_bounds = self.visual_subtree_bounds(*child_id);
            if !visible {
                // 先取消隐藏子树的指针交互，焦点在所有门控更新后统一处理。
                self.cancel_pointer_state_in_subtree(*child_id);
            }
            if let Some(child) = self.get_mut(*child_id) {
                child.set_parent_visible(*visible);
            }
            self.tree_version = self.tree_version.wrapping_add(1);
            let new_bounds = self.visual_subtree_bounds(*child_id);
            for rect in [old_bounds, new_bounds].into_iter().flatten() {
                if rect.w > 0.0 && rect.h > 0.0 {
                    self.push_paint_invalidation(*child_id, Some(rect));
                }
            }
        }
        // 仅在旧焦点因门控变化失效时执行迁移或通用清理。
        if let Some(focused) = focused_before.filter(|id| !self.focus_target_available(*id)) {
            // 通过 System 私有边界查询具体组件的替代焦点约定。
            let replacement =
                crate::ui::tree_widget_hooks::focus_replacement_after_child_visibility(
                    self, focused, changes,
                );
            // 触发完整的 FocusOut/FocusIn 生命周期。
            self.set_focus(replacement);
        }
        !changes.is_empty()
    }

    pub(crate) fn rebuild_widget_overlays(&mut self) {
        let previous_trap_owners: Vec<_> = self
            .overlay_stack
            .iter()
            .filter(|entry| entry.traps_focus())
            .map(|entry| entry.owner())
            .collect();
        let previous_widget_overlays: Vec<_> = self
            .overlay_stack
            .iter()
            .filter(|entry| !entry.is_managed())
            .map(|entry| {
                (
                    entry.owner(),
                    entry.kind(),
                    entry.bounds_rect(),
                    entry.z_index_value(),
                    entry.is_modal(),
                    entry.dismisses_on_outside(),
                    entry.traps_focus(),
                    // backdrop 请求属于 overlay 拓扑与效果失效事实。
                    entry.backdrop_blur_value(),
                )
            })
            .collect();
        // 从当前根布局矩形提取逻辑表面，避免复用上一帧窗口尺寸。
        let overlay_surface = self
            // 获取当前组件树根节点。
            .root_id
            // 读取根节点的最新布局结果。
            .and_then(|root_id| self.get(root_id))
            // 将根节点尺寸归一到表面坐标原点。
            .map(|root| Rect::new(0.0, 0.0, root.frame().w.max(0.0), root.frame().h.max(0.0)))
            // 无根节点时使用空表面。
            .unwrap_or_default();
        // 以当前表面重建所有组件声明的浮层登记。
        let entries: Vec<_> = self
            .traverse()
            .iter()
            .copied()
            .filter_map(|id| {
                if !self.is_effectively_visible(id) || self.is_pending_removal_subtree(id) {
                    return None;
                }
                let node = self.get(id)?;
                // 显式传入当前表面，使边界敏感浮层与同帧绘制几何一致。
                let mut entry =
                    node.overlay_entry_for_surface(id, node.frame(), overlay_surface)?;
                if let Some(bounds) = entry.bounds_rect() {
                    entry = entry.bounds(self.node_visual_rect(id, bounds)?);
                }
                Some(entry)
            })
            .collect();
        let widget_overlays_changed = !previous_widget_overlays.iter().copied().eq(entries
            .iter()
            .filter(|entry| !entry.is_managed())
            .map(|entry| {
                (
                    entry.owner(),
                    entry.kind(),
                    entry.bounds_rect(),
                    entry.z_index_value(),
                    entry.is_modal(),
                    entry.dismisses_on_outside(),
                    entry.traps_focus(),
                    // 请求变化必须触发同帧 effect 重解析与合成失效。
                    entry.backdrop_blur_value(),
                )
            }));

        self.overlay_stack
            .retain_entries(|entry| entry.is_managed());
        for entry in entries {
            self.overlay_stack.push_entry(entry);
        }

        for owner in previous_trap_owners {
            if !self
                .overlay_stack
                .iter()
                .any(|entry| entry.traps_focus() && entry.owner() == owner)
            {
                self.restore_focus_after_trap_owner(owner);
            }
        }

        if widget_overlays_changed {
            // Overlay membership changes the compositor's clipping/cache topology.
            self.tree_version = self.tree_version.wrapping_add(1);
            self.mark_full_frame_composite();
        }
    }

    pub(crate) fn reconcile_lifecycle_after_layout(&mut self) {
        self.cancel_hidden_interaction();
        let mut states = std::mem::take(&mut self.lifecycle_states_scratch);
        states.clear();
        states.extend(
            self.traverse()
                .iter()
                .copied()
                .filter(|&id| self.get(id).is_some())
                .map(|id| {
                    (
                        id,
                        self.visible_rect_for(id).is_some() || self.focus_affects_active(id),
                    )
                }),
        );

        for (id, should_be_active) in states.iter().copied() {
            let mut mounted_now = false;
            if let Some(node) = self.get_mut(id) {
                if !node.mounted() {
                    node.set_mounted(true);
                    node.on_mount();
                    mounted_now = true;
                }

                if should_be_active && !node.active() {
                    node.set_active(true);
                    node.on_active();
                } else if !should_be_active && node.active() {
                    node.set_active(false);
                    node.on_inactive();
                }
            }
            if mounted_now {
                self.register_app_state_snapshot(id);
            }
        }
        self.lifecycle_states_scratch = states;
    }

    pub(crate) fn sync_app_state_registry_from_lifecycle_states(&self) {
        if self.app_state.is_none() {
            return;
        }
        for &(id, _) in &self.lifecycle_states_scratch {
            if self.get(id).is_some_and(|node| node.mounted()) {
                self.register_app_state_snapshot(id);
            }
        }
    }

    pub(crate) fn focus_affects_active(&self, id: WidgetId) -> bool {
        let mut current = self.managers().focus.focused_widget();
        while let Some(current_id) = current {
            if current_id == id {
                return true;
            }
            current = self.get(current_id).and_then(|node| node.parent());
        }
        false
    }

    pub(crate) fn visible_rect_for(&self, id: WidgetId) -> Option<Rect> {
        self.visible_visual_rect_for(id)
    }

    /// 自下而上扩展：当子节点右侧/底部超出容器时，扩展容器宽度/高度。
    /// 后序遍历确保子节点先扩展、父节点后扩展。
    /// 返回是否有任何容器被扩展，并把本趟签名追加到调用方复用缓冲。
    pub(crate) fn layout_expand(
        &mut self,
        order: &[WidgetId],
        expand_sig: &mut Vec<(WidgetId, i32, i32, i32, i32)>,
        resized_children: &mut HashSet<WidgetId>,
        children: &mut Vec<WidgetId>,
    ) -> bool {
        let mut any_resized = false;
        // 收集本趟中被扩展过的子节点，用于触发其父容器重排
        resized_children.clear();
        for &id in order.iter().rev() {
            if !self.is_effectively_visible(id) {
                continue;
            }
            // 根已有确定客户区高度时不得被内容撑开（窗口缩小场景）；
            // bootstrap（高度仍 ≤1）仍允许 expand，以便无窗口尺寸时由子项撑开。
            if self.root_id == Some(id) {
                let root_h = self.get(id).map(|n| n.frame().h).unwrap_or(0.0);
                if root_h > 1.0 {
                    continue;
                }
            }
            let (prevents_child_expansion, node_frame) = match self.get(id) {
                Some(n) if !n.children().is_empty() => {
                    children.clear();
                    children.extend_from_slice(n.children());
                    (
                        n.children_clip(n.frame()).is_some() || !n.child_overflow_expands_parent(),
                        n.frame(),
                    )
                }
                _ => continue,
            };
            // Viewport 与显式定位容器不由视觉溢出的子树反向撑开。
            if prevents_child_expansion {
                continue;
            }

            // 检查是否有直接子节点在本趟中被扩展过
            let has_resized_child = children.iter().any(|cid| {
                // out-of-flow 子项扩展不推动正常流父容器重新分配槽位。
                self.get(*cid).is_some_and(|child| {
                    // 只有正常流子项的实际扩展需要重排兄弟。
                    !child.position().mode.is_out_of_flow() && resized_children.contains(cid)
                })
            });

            // 取所有可见子节点的最大右/下边界
            let mut max_right = node_frame.x + node_frame.w;
            let mut max_bottom = node_frame.y + node_frame.h;
            for &cid in children.iter() {
                if let Some(child) = self.get(cid) {
                    if child.visible() && !child.position().mode.is_out_of_flow() {
                        let cf = child.frame();
                        let child_right = cf.x + cf.w;
                        let child_bottom = cf.y + cf.h;
                        let rel_right = (cf.x - node_frame.x) + cf.w;
                        let rel_bottom = (cf.y - node_frame.y) + cf.h;
                        // 只考虑延伸到可见区域的子节点（防止滚动到视口上方时无限膨胀）
                        let child_extends_right_of_parent = cf.x + cf.w > node_frame.x;
                        let child_extends_below_parent = cf.y + cf.h > node_frame.y;
                        if child_right > 0.0
                            && child_extends_right_of_parent
                            && rel_right > node_frame.w
                        {
                            max_right = max_right.max(child_right);
                        }
                        if child_bottom > 0.0
                            && child_extends_below_parent
                            && rel_bottom > node_frame.h
                        {
                            max_bottom = max_bottom.max(child_bottom);
                        }
                    }
                }
            }

            let new_w = max_right - node_frame.x;
            let new_h = max_bottom - node_frame.y;
            let needs_relayout =
                new_w > node_frame.w + 0.5 || new_h > node_frame.h + 0.5 || has_resized_child;
            if needs_relayout {
                let old_frame = node_frame;
                // 非根节点默认不得超过父级已分配 frame，避免窗口缩小后中间层撑破客户区。
                // 滚动轴的放行范围覆盖整个 viewport 子树，而不只是直接子节点。
                // 中间的 Container/Space 不能把内容重新限制回 viewport 的 frame，
                // 否则嵌套内容的扩展无法传递到 ScrollView::content_bounds。
                let (scrolls_horizontally, scrolls_vertically) = self
                    .nearest_viewport_overflow_axes(id)
                    .unwrap_or((false, false));
                let parent_frame = if self.root_id == Some(id) {
                    None
                } else {
                    self.get(id)
                        .and_then(|n| n.parent())
                        .and_then(|pid| self.get(pid).map(|p| p.frame()))
                };
                let parent_cap_w = parent_frame
                    .filter(|_| !scrolls_horizontally)
                    .map(|pf| (pf.x + pf.w - old_frame.x).max(0.0));
                let parent_cap_h = parent_frame
                    .filter(|_| !scrolls_vertically)
                    .map(|pf| (pf.y + pf.h - old_frame.y).max(0.0));
                let mut effective_w = new_w.max(node_frame.w);
                let mut effective_h = new_h.max(node_frame.h);
                if let Some(cap) = parent_cap_w {
                    effective_w = effective_w.min(cap);
                }
                if let Some(cap) = parent_cap_h {
                    effective_h = effective_h.min(cap);
                }
                // 显式定宽/定高是硬约束：Phase 2 不得再撑开，否则与 Phase 1 分配打架。
                // （内容可溢出/裁剪；滚动尺寸增长只发生在无固定边的内容根上。）
                let (lock_w, lock_h) = self.phase2_explicit_size_locks(id);
                if lock_w {
                    effective_w = node_frame.w;
                }
                if lock_h {
                    effective_h = node_frame.h;
                }
                let expanded_w = effective_w > node_frame.w + 0.5;
                let expanded_h = effective_h > node_frame.h + 0.5;
                if expanded_w || expanded_h {
                    tracing::debug!(
                        "[Layout] Phase 2: id={} frame ({:.0},{:.0}) → ({:.0},{:.0}) (child right/bottom=({:.0},{:.0}))",
                        id,
                        node_frame.w,
                        node_frame.h,
                        effective_w,
                        effective_h,
                        max_right,
                        max_bottom,
                    );
                    if self.set_layout_frame(
                        id,
                        Rect::new(old_frame.x, old_frame.y, effective_w, effective_h),
                    ) {
                        any_resized = true;
                        resized_children.insert(id);
                        expand_sig.push((
                            id,
                            node_frame.w.round() as i32,
                            node_frame.h.round() as i32,
                            effective_w.round() as i32,
                            effective_h.round() as i32,
                        ));
                        #[cfg(test)]
                        {
                            self.layout_expand_ops
                                .set(self.layout_expand_ops.get().wrapping_add(1));
                        }
                    }
                } else if has_resized_child {
                    tracing::debug!(
                        "[Layout] Phase 2: id={} re-layout siblings (child resized, frame=({:.0},{:.0}))",
                        id,
                        node_frame.w,
                        node_frame.h,
                    );
                }
                // 重新布局子节点（容器扩展后 or 子节点被扩展过）。
                // 对已扩展子节点用当前 frame 做 measure 下限，避免父级仍按旧
                // measured_size 把扩展写回（120↔124 振荡）。
                let relayout_frame = if expanded_w || expanded_h {
                    Rect::new(old_frame.x, old_frame.y, effective_w, effective_h)
                } else {
                    old_frame
                };
                let new_positions = self.layout_children_preserving_expansions(
                    id,
                    relayout_frame,
                    children,
                    resized_children,
                );
                let mut child_moved = false;
                for (child_id, rect) in new_positions {
                    if self.set_layout_frame(child_id, rect) {
                        child_moved = true;
                        resized_children.insert(child_id);
                    }
                }
                // 仅在 frame 实际变化时计为 progress，避免「结果不变的 sibling re-layout」空转收敛循环
                if child_moved {
                    any_resized = true;
                    resized_children.insert(id);
                }
            }
        }
        any_resized
    }
}
