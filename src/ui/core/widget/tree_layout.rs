use super::super::*;
use super::WidgetTree;
use crate::core::{Constraints, Rect, Size};
use std::collections::HashSet;

#[cfg(test)]
use super::LAYOUT_TRACE_PHASE;

fn frame_constraints(frame: Rect) -> Constraints {
    Constraints::loose(Size::new(frame.w, frame.h))
}

#[derive(Default)]
struct LayoutTraversalScratch {
    roots: HashSet<WidgetId>,
    paths: HashSet<WidgetId>,
    stack: Vec<(WidgetId, bool)>,
}

impl WidgetTree {
    /// 返回最近 viewport 祖先允许内容溢出的轴。
    ///
    /// 最近 viewport 决定当前内容坐标系：嵌套 ScrollView 时不能越过内层
    /// viewport，错误地继承外层的滚动方向。未知 viewport 保守地禁止溢出。
    fn nearest_viewport_overflow_axes(&self, id: WidgetId) -> Option<(bool, bool)> {
        let mut current = id;
        while let Some(parent_id) = self.get(current).and_then(|node| node.parent()) {
            let parent = self.get(parent_id)?;
            if parent.children_clip(parent.frame()).is_some() {
                let axes = parent
                    .component()
                    .as_any()
                    .downcast_ref::<crate::ui::widgets::ScrollView>()
                    .and_then(|scroll_view| match scroll_view.snapshot_fields() {
                        crate::ui::SnapshotFields::ScrollView { direction, .. } => {
                            Some((direction.can_scroll_x(), direction.can_scroll_y()))
                        }
                        _ => None,
                    })
                    .unwrap_or((false, false));
                return Some(axes);
            }
            current = parent_id;
        }
        None
    }

    /// Phase 2 不得撑开显式定宽/定高的节点（Container/Space/ScrollView 等）。
    fn phase2_explicit_size_locks(&self, id: WidgetId) -> (bool, bool) {
        let Some(node) = self.get(id) else {
            return (false, false);
        };
        match node.component().snapshot_fields() {
            crate::ui::SnapshotFields::Container { style }
            | crate::ui::SnapshotFields::Grid { style, .. } => {
                (style.width.is_some(), style.height.is_some())
            }
            crate::ui::SnapshotFields::Space {
                fixed_width,
                fixed_height,
                ..
            } => (fixed_width.is_some(), fixed_height.is_some()),
            crate::ui::SnapshotFields::ScrollView {
                fixed_width,
                fixed_height,
                ..
            } => (fixed_width.is_some(), fixed_height.is_some()),
            _ => (false, false),
        }
    }

    /// 返回 Layout 失效影响的子树先序遍历顺序。
    ///
    /// 全帧或含 Layout 根时遍历对应子树；无 Layout 失效时返回空（跳过 layout）。
    pub fn layout_traverse(&self) -> Vec<ComponentId> {
        let mut result = Vec::new();
        self.fill_layout_traversal(&mut result, &mut LayoutTraversalScratch::default());
        result
    }

    fn fill_layout_traversal(
        &self,
        result: &mut Vec<ComponentId>,
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

    pub fn layout(&mut self) {
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

        let mut order = Vec::new();
        let mut traversal_scratch = LayoutTraversalScratch::default();
        self.fill_layout_traversal(&mut order, &mut traversal_scratch);
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
                            && self.get(cid).is_some_and(|c| {
                                c.visible() && (c.frame().w <= 0.0 || c.frame().h <= 0.0)
                            })
                    })
                })
            });
            if !needs_bootstrap {
                return;
            }
            if let Some(root_id) = self.root_id {
                self.push_layout_invalidation(root_id);
            }
            self.fill_layout_traversal(&mut order, &mut traversal_scratch);
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
        let rev_order: Vec<WidgetId> = order.iter().rev().copied().collect();
        // 安全网：若连续两轮 Phase 2 扩展签名完全相同（同 id、同 before/after），
        // 视为无 progress，停止空转（根因仍应在 measure；此处防止打满 max_passes）。
        let mut prev_expand_sig: Option<Vec<(WidgetId, i32, i32, i32, i32)>> = None;
        for _converge_pass in 0..max_passes {
            #[cfg(test)]
            {
                converge_passes += 1;
            }
            let mut any_change = false;
            let mut pass_expand_sig: Vec<(WidgetId, i32, i32, i32, i32)> = Vec::new();

            // Phase 1: Top-down — 父容器根据当前 frame 为子节点分配位置
            #[cfg(test)]
            LAYOUT_TRACE_PHASE.with(|p| p.set(1));
            for &id in &order {
                let positions: Vec<(WidgetId, Rect)> = {
                    let node = match self.get(id) {
                        Some(n) => n,
                        None => continue,
                    };
                    let frame = node.frame();
                    let children: Vec<WidgetId> = node.children().to_vec();
                    if children.is_empty() {
                        continue;
                    }
                    node.layout_children(frame, &children, self)
                };
                for (child_id, rect) in positions {
                    if self.set_layout_frame(child_id, rect) {
                        any_change = true;
                    }
                }
            }
            #[cfg(test)]
            LAYOUT_TRACE_PHASE.with(|p| p.set(0));

            // 内循环：交替扩展和收缩直到稳定
            for _inner_pass in 0..3 {
                #[cfg(test)]
                LAYOUT_TRACE_PHASE.with(|p| p.set(2));
                let (expanded, sig) = self.layout_expand(&rev_order);
                pass_expand_sig.extend(sig);
                #[cfg(test)]
                LAYOUT_TRACE_PHASE.with(|p| p.set(4));
                let shrunk = self.layout_shrink(&rev_order);
                #[cfg(test)]
                LAYOUT_TRACE_PHASE.with(|p| p.set(0));
                if expanded || shrunk {
                    any_change = true;
                }
                if !expanded && !shrunk {
                    break;
                }
            }

            // Phase 3: 更新 viewport 容器的 content_bounds
            self.layout_viewports(&order);

            if !pass_expand_sig.is_empty() {
                if prev_expand_sig.as_ref() == Some(&pass_expand_sig) {
                    crate::core::log::debug_fn(
                        "[Layout] Phase 2: identical expand signature — stop (no progress)",
                    );
                    break;
                }
                prev_expand_sig = Some(pass_expand_sig);
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
        self.layout_viewports(&order);
        self.refresh_virtual_scroll_children();
        // Phase 6：layout 完成后用最新 frame 绑定 State → Paint rect
        self.bind_reactive_widget_states();
        self.rebuild_widget_overlays();
        self.reconcile_lifecycle_after_layout();
        self.sync_app_state_registry();
        crate::core::log::debug_fn("[Layout] layout() done");
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
                )
            })
            .collect();
        let entries: Vec<_> = self
            .traverse()
            .iter()
            .copied()
            .filter_map(|id| {
                let node = self.get(id)?;
                if !node.visible() {
                    return None;
                }
                node.overlay_entry(id, node.frame())
            })
            .collect();
        let widget_overlays_changed = previous_widget_overlays
            != entries
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
                    )
                })
                .collect::<Vec<_>>();

        self.overlay_stack
            .retain_entries(|entry| entry.is_managed());
        for entry in entries {
            self.overlay_stack.push_entry(entry);
        }

        let active_trap_owners: Vec<_> = self
            .overlay_stack
            .iter()
            .filter(|entry| entry.traps_focus())
            .map(|entry| entry.owner())
            .collect();
        for owner in previous_trap_owners {
            if !active_trap_owners.contains(&owner) {
                self.restore_focus_after_trap_owner(owner);
            }
        }

        if widget_overlays_changed {
            // Overlay membership changes the compositor's clipping/cache topology.
            self.tree_version = self.tree_version.wrapping_add(1);
            self.mark_full_frame_dirty();
        }
    }

    pub(crate) fn reconcile_lifecycle_after_layout(&mut self) {
        self.cancel_hidden_interaction();
        let states: Vec<(WidgetId, bool)> = self
            .traverse()
            .iter()
            .copied()
            .filter(|&id| self.get(id).is_some())
            .map(|id| {
                (
                    id,
                    self.visible_rect_for(id).is_some() || self.focus_affects_active(id),
                )
            })
            .collect();

        for (id, should_be_active) in states {
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
    }

    fn focus_affects_active(&self, id: WidgetId) -> bool {
        let mut current = self.managers().focus.focused_component();
        while let Some(current_id) = current {
            if current_id == id {
                return true;
            }
            current = self.get(current_id).and_then(|node| node.parent());
        }
        false
    }

    pub(crate) fn visible_rect_for(&self, id: WidgetId) -> Option<Rect> {
        let node = self.get(id)?;
        if !node.visible() {
            return None;
        }
        let frame = node.frame();
        let is_overlay = node.overlay_entry(id, frame).is_some();
        let mut rect = if frame.w > 0.0 && frame.h > 0.0 {
            frame
        } else {
            node.hit_test_frame(frame)
        };
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return None;
        }

        // 零布局槽的浮动控件以真实命中区域作为语义边界；浮层本身不受祖先
        // viewport 裁剪，否则视觉已提升到浮层而自动化边界仍会被内容区截断。
        if is_overlay {
            return Some(rect);
        }

        let mut current = id;
        while let Some(parent_id) = self.get(current).and_then(|n| n.parent()) {
            let parent = self.get(parent_id)?;
            if !parent.visible() {
                return None;
            }
            if let Some((sx, sy)) = parent.viewport_scroll_offset() {
                rect = Rect::new(rect.x - sx, rect.y - sy, rect.w, rect.h);
            }
            if let Some(clip) = parent.children_clip(parent.frame()) {
                rect = rect.intersect(&clip)?;
            }
            current = parent_id;
        }

        Some(rect)
    }

    /// 自下而上扩展：当子节点右侧/底部超出容器时，扩展容器宽度/高度。
    /// 后序遍历确保子节点先扩展、父节点后扩展。
    /// 返回 (是否有任何容器被扩展, 本趟扩展签名)。
    fn layout_expand(
        &mut self,
        rev_order: &[WidgetId],
    ) -> (bool, Vec<(WidgetId, i32, i32, i32, i32)>) {
        let mut any_resized = false;
        let mut expand_sig: Vec<(WidgetId, i32, i32, i32, i32)> = Vec::new();
        // 收集本趟中被扩展过的子节点，用于触发其父容器重排
        let mut resized_children = std::collections::HashSet::new();
        for &id in rev_order {
            // 根已有确定客户区高度时不得被内容撑开（窗口缩小场景）；
            // bootstrap（高度仍 ≤1）仍允许 expand，以便无窗口尺寸时由子项撑开。
            if self.root_id == Some(id) {
                let root_h = self.get(id).map(|n| n.frame().h).unwrap_or(0.0);
                if root_h > 1.0 {
                    continue;
                }
            }
            let (children, prevents_child_expansion, node_frame) = match self.get(id) {
                Some(n) if !n.children().is_empty() => (
                    n.children().to_vec(),
                    n.children_clip(n.frame()).is_some() || !n.child_overflow_expands_parent(),
                    n.frame(),
                ),
                _ => continue,
            };
            // Viewport 与显式定位容器不由视觉溢出的子树反向撑开。
            if prevents_child_expansion {
                continue;
            }

            // 检查是否有直接子节点在本趟中被扩展过
            let has_resized_child = children.iter().any(|cid| resized_children.contains(cid));

            // 取所有可见子节点的最大右/下边界
            let mut max_right = node_frame.x + node_frame.w;
            let mut max_bottom = node_frame.y + node_frame.h;
            for &cid in &children {
                if let Some(child) = self.get(cid) {
                    if child.visible() {
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
                    crate::core::log::debug_fn(format!(
                        "[Layout] Phase 2: id={} frame ({:.0},{:.0}) → ({:.0},{:.0}) (child right/bottom=({:.0},{:.0}))",
                        id,
                        node_frame.w,
                        node_frame.h,
                        effective_w,
                        effective_h,
                        max_right,
                        max_bottom,
                    ));
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
                    crate::core::log::debug_fn(format!(
                        "[Layout] Phase 2: id={} re-layout siblings (child resized, frame=({:.0},{:.0}))",
                        id, node_frame.w, node_frame.h,
                    ));
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
                    &children,
                    &resized_children,
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
        (any_resized, expand_sig)
    }

    /// Phase 2 重排：对已扩展子节点，measure 结果不得低于当前 frame。
    fn layout_children_preserving_expansions(
        &self,
        id: WidgetId,
        frame: Rect,
        children: &[WidgetId],
        expanded: &std::collections::HashSet<WidgetId>,
    ) -> Vec<(WidgetId, Rect)> {
        let Some(node) = self.get(id) else {
            return Vec::new();
        };
        let Some(layout) = node.component().as_layout() else {
            return Vec::new();
        };
        let mut measured = layout.measure_children(frame, children, self);
        for child in &mut measured {
            if !expanded.contains(&child.id) {
                continue;
            }
            if let Some(cf) = self.get(child.id).map(|n| n.frame()) {
                child.measured_size.w = child.measured_size.w.max(cf.w);
                child.measured_size.h = child.measured_size.h.max(cf.h);
            }
        }
        layout.layout_children(frame, &measured, self)
    }

    /// 更新所有 viewport 容器的 content_bounds。
    /// 只触发 content_bounds 副作用，不移动子节点位置。
    fn layout_viewports(&mut self, order: &[WidgetId]) {
        for &id in order {
            if let Some(node) = self.get(id) {
                if node.children_clip(node.frame()).is_none() {
                    continue;
                }
                let frame = node.frame();
                let children = node.children().to_vec();
                if children.is_empty() {
                    continue;
                }
                crate::core::log::debug_fn(format!(
                    "[Layout] Phase 3: viewport id={} frame=({:.0},{:.0},{:.0},{:.0}) {} children",
                    id,
                    frame.x,
                    frame.y,
                    frame.w,
                    frame.h,
                    children.len(),
                ));
                // 仅触发 content_bounds 副作用，丢弃返回的 child rects
                let _ = node.layout_children(frame, &children, self);
            }
        }
    }

    /// Rebuild VirtualScroll child windows when layout frame or scroll offset changes.
    fn refresh_virtual_scroll_children(&mut self) {
        let ids: Vec<_> = self.traverse().iter().copied().collect();
        for id in ids {
            let viewport_h = self.get(id).map(|node| node.frame().h).unwrap_or(0.0);
            if viewport_h <= 0.0 {
                continue;
            }
            self.refresh_virtual_scroll_component(id, Some(viewport_h));
        }
    }

    /// 检查节点是否有 viewport 祖先（如 ScrollView）。
    /// 递归遍历祖先链，不限于直接父节点。
    /// 用于 layout_shrink 中避免收缩 viewport 内部节点，防止与 ScrollView 尺寸设定形成振荡。
    fn has_viewport_ancestor(&self, id: WidgetId) -> bool {
        let mut current = id;
        while let Some(pid) = self.get(current).and_then(|n| n.parent()) {
            if self
                .get(pid)
                .map(|p| p.children_clip(p.frame()).is_some())
                .unwrap_or(false)
            {
                return true;
            }
            current = pid;
        }
        false
    }

    /// 父级当前会分配给 `id` 的 frame（Phase 1 槽位）。
    /// Phase 4 不得收缩到该高度以下，否则 Stretch/flex 分配会被下一轮 Phase 1 拉回，形成 thrashing。
    fn parent_allocated_frame(&self, id: WidgetId) -> Option<Rect> {
        let parent_id = self.get(id).and_then(|n| n.parent())?;
        let parent_frame = self.get(parent_id)?.frame();
        let children = self.get(parent_id)?.children().to_vec();
        if children.is_empty() {
            return None;
        }
        let positions = self
            .get(parent_id)?
            .layout_children(parent_frame, &children, self);
        positions
            .into_iter()
            .find(|(cid, _)| *cid == id)
            .map(|(_, rect)| rect)
    }

    /// 收缩过大的容器。与 layout_expand 相反——当子节点高度
    /// 显著小于容器当前高度，且子节点延伸到可见区域时，收缩容器。
    /// 每轮先重新布局子节点（确保兄弟组件靠拢），再检查是否需要收缩。
    /// 返回是否有任何容器被收缩。
    fn layout_shrink(&mut self, rev_order: &[WidgetId]) -> bool {
        let mut any_changed = false;
        for _pass in 0..3 {
            let mut pass_changed = false;
            // Phase A: 收集需要收缩的容器
            #[derive(Clone)]
            struct ShrinkOp {
                id: WidgetId,
                needed_h: f32,
            }
            let mut ops: Vec<ShrinkOp> = Vec::new();

            for &id in rev_order {
                // 根 frame 由窗口客户区锁定，shrink 同样不得改写。
                if self.root_id == Some(id) {
                    continue;
                }
                let is_viewport = self
                    .get(id)
                    .map(|n| n.children_clip(n.frame()).is_some())
                    .unwrap_or(false);
                if is_viewport {
                    continue;
                }
                // 不收缩祖先链中有 viewport（如 ScrollView）的节点，
                // 避免与 ScrollView::layout_children 的尺寸设定形成振荡。
                // 递归检查所有祖先，不限于直接父节点（修复 Container→Input 嵌套场景）。
                if self.has_viewport_ancestor(id) {
                    continue;
                }
                // layout_viewports（Phase 3）会在收缩后更新 content_bounds。

                let children: Vec<WidgetId> = match self.get(id) {
                    Some(n) if !n.children().is_empty() => n.children().to_vec(),
                    _ => continue,
                };

                // 先按当前 frame 重新布局子节点（兄弟组件靠拢/张开）
                let Some(frame) = self.get(id).map(|n| n.frame()) else {
                    continue;
                };
                let positions: Vec<(WidgetId, Rect)> = {
                    let Some(node) = self.get(id) else {
                        continue;
                    };
                    node.layout_children(frame, &children, self)
                };
                for (child_id, rect) in positions {
                    if self.set_layout_frame(child_id, rect) {
                        pass_changed = true;
                    }
                }

                // 检查容器是否需要收缩
                let Some(node_frame) = self.get(id).map(|n| n.frame()) else {
                    continue;
                };
                let mut max_child_bottom = f32::MIN;
                let mut has_visible = false;
                for &cid in &children {
                    if let Some(child) = self.get(cid) {
                        if child.visible() {
                            let cf = child.frame();
                            let child_bottom = cf.y + cf.h;
                            if child_bottom > 0.0 {
                                max_child_bottom = max_child_bottom.max(child_bottom);
                                has_visible = true;
                            }
                        }
                    }
                }
                if !has_visible {
                    continue;
                }

                let needed_h = max_child_bottom - node_frame.y;
                // ⭐ 最小高度取子节点实际内容和 measure 的较大值。
                // 设此下限可防止收缩到子节点内容以下，从而避免与
                // layout_expand（Phase 2）形成振荡循环。
                // 使用 1.0 像素绝对最小值而非比例值（如 0.01 * h），
                // 后者在高 DPI 场景下可能过大（2000px * 0.01 = 20px 虚高）。
                let measure_constraints = frame_constraints(node_frame);
                let pref_h = self
                    .get(id)
                    .map(|n| n.measure(measure_constraints).h)
                    .unwrap_or(0.0);
                let min_h = needed_h.max(pref_h).max(1.0);
                // 不得低于父级 Phase 1 分配高度（Stretch / flex-grow 槽位）。
                // demo 侧栏 column_fit 被 row Stretch 拉到客户区高后，若按内容缩回，
                // 下一轮 Phase 1 会再次拉满 → 同结果 Phase 4 空转 thrashing。
                let parent_floor_h = self.parent_allocated_frame(id).map(|r| r.h).unwrap_or(0.0);
                let effective_needed = min_h.max(parent_floor_h);
                if node_frame.h - effective_needed > 0.5 {
                    crate::core::log::debug_fn(format!(
                        "[Layout] Phase 4: id={} shrink {:.0}px {:.0}→{:.0} (needed={:.0} pref={:.0} floor={:.0})",
                        id,
                        node_frame.h - effective_needed,
                        node_frame.h,
                        effective_needed,
                        needed_h,
                        pref_h,
                        parent_floor_h,
                    ));
                    ops.push(ShrinkOp {
                        id,
                        needed_h: effective_needed,
                    });
                }
            }
            // Phase B: 执行收缩
            for op in &ops {
                if let Some(old_frame) = self.get(op.id).map(|n| n.frame()) {
                    if !self.set_layout_frame(
                        op.id,
                        Rect::new(old_frame.x, old_frame.y, old_frame.w, op.needed_h),
                    ) {
                        continue;
                    }
                    #[cfg(test)]
                    {
                        self.layout_shrink_ops
                            .set(self.layout_shrink_ops.get().wrapping_add(1));
                    }
                    let children: Vec<WidgetId> = self
                        .get(op.id)
                        .map(|n| n.children().to_vec())
                        .unwrap_or_default();
                    let new_frame = Rect::new(old_frame.x, old_frame.y, old_frame.w, op.needed_h);
                    // 收缩后重新布局子节点
                    let new_positions = self
                        .get(op.id)
                        .map(|n| n.layout_children(new_frame, &children, self))
                        .unwrap_or_default();
                    for (child_id, rect) in new_positions {
                        let _ = self.set_layout_frame(child_id, rect);
                    }
                    // 重新布局父容器，让兄弟组件靠拢
                    if let Some(pid) = self.get(op.id).and_then(|n| n.parent()) {
                        let parent_frame = self.get(pid).map(|n| n.frame()).unwrap_or_default();
                        let parent_children: Vec<WidgetId> = self
                            .get(pid)
                            .map(|n| n.children().to_vec())
                            .unwrap_or_default();
                        if !parent_children.is_empty() {
                            let parent_positions = self
                                .get(pid)
                                .map(|n| n.layout_children(parent_frame, &parent_children, self))
                                .unwrap_or_default();
                            for (child_id, rect) in parent_positions {
                                let _ = self.set_layout_frame(child_id, rect);
                            }
                        }
                    }
                    pass_changed = true;
                }
            }
            if !pass_changed {
                break;
            }
            any_changed = true;
        }
        any_changed
    }

    pub fn update(&mut self, dt: f64) -> bool {
        self.update_animations(dt)
            .into_iter()
            .any(|(_, still_active)| still_active)
    }

    pub(crate) fn update_animations(&mut self, dt: f64) -> Vec<(WidgetId, bool)> {
        let ids = self.animation_node_ids();
        self.update_animation_nodes(ids, dt)
    }

    pub(crate) fn update_animations_except<I>(
        &mut self,
        excluded_ids: I,
        dt: f64,
    ) -> Vec<(WidgetId, bool)>
    where
        I: IntoIterator<Item = WidgetId>,
    {
        let excluded_ids: Vec<_> = excluded_ids.into_iter().collect();
        let ids: Vec<_> = self
            .animation_node_ids()
            .into_iter()
            .filter(|id| !excluded_ids.contains(id))
            .collect();
        self.update_animation_nodes(ids, dt)
    }

    fn animation_node_ids(&self) -> Vec<WidgetId> {
        self.traverse()
            .iter()
            .copied()
            .filter(|&id| self.active_animation_frame(id).is_some())
            .collect()
    }

    fn active_animation_frame(&self, id: WidgetId) -> Option<Rect> {
        let node = self.get(id)?;
        (node.visible()
            && node.active()
            && node
                .capabilities()
                .contains(crate::ui::traits::WidgetCapabilities::ANIMATION))
        .then_some(node.frame())
    }

    pub(crate) fn update_animation_nodes<I>(&mut self, ids: I, dt: f64) -> Vec<(WidgetId, bool)>
    where
        I: IntoIterator<Item = WidgetId>,
    {
        let mut updates = Vec::new();
        let mut widget_overlays_changed = false;
        for id in ids {
            let Some(frame) = self.active_animation_frame(id) else {
                updates.push((id, false));
                continue;
            };

            let Some((still_active, dirty)) = self.get_mut(id).and_then(|node| {
                let animation = node.component_mut().as_animation_mut()?;
                let still_active = animation.update_animation(dt);
                let dirty = animation.dirty_bounds(frame);
                Some((still_active, dirty))
            }) else {
                updates.push((id, false));
                continue;
            };

            if dirty.w > 0.0 && dirty.h > 0.0 {
                self.invalidate_paint_rect(id, dirty);
            }
            widget_overlays_changed |= !self.widget_overlay_is_current(id);
            updates.push((id, still_active));
        }
        self.cancel_hidden_interaction();
        if widget_overlays_changed || updates.iter().any(|(_, still_active)| !still_active) {
            self.rebuild_widget_overlays();
        }
        updates
    }

    fn widget_overlay_is_current(&self, id: WidgetId) -> bool {
        let desired = self
            .get(id)
            .filter(|node| node.visible())
            .and_then(|node| node.overlay_entry(id, node.frame()));
        let current: Vec<_> = self
            .overlay_stack
            .iter()
            .filter(|entry| !entry.is_managed() && entry.owner() == id)
            .collect();

        match (desired, current.as_slice()) {
            (None, []) => true,
            (Some(desired), [current]) => {
                desired.kind() == current.kind()
                    && desired.bounds_rect() == current.bounds_rect()
                    && desired.z_index_value() == current.z_index_value()
                    && desired.is_modal() == current.is_modal()
                    && desired.dismisses_on_outside() == current.dismisses_on_outside()
                    && desired.traps_focus() == current.traps_focus()
            }
            _ => false,
        }
    }
}
