use super::super::*;
use super::WidgetTree;
use crate::core::{Constraints, Rect};

impl WidgetTree {
    /// 返回 Layout 失效影响的子树先序遍历顺序。
    ///
    /// 全帧或含 Layout 根时遍历对应子树；无 Layout 失效时返回空（跳过 layout）。
    pub fn layout_traverse(&self) -> Vec<ComponentId> {
        let inv = self.invalidation.lock().unwrap_or_else(|e| e.into_inner());
        if inv.needs_full_frame() {
            return self.traverse();
        }
        let layout_roots = inv.layout_roots();
        drop(inv);
        if layout_roots.is_empty() {
            return Vec::new();
        }

        let mut needed = std::collections::HashSet::new();
        for &id in &layout_roots {
            let mut cur = Some(id);
            while let Some(cid) = cur {
                needed.insert(cid);
                cur = self.get(cid).and_then(|n| n.parent());
            }
            let mut stack = vec![id];
            while let Some(nid) = stack.pop() {
                needed.insert(nid);
                if let Some(node) = self.get(nid) {
                    for &c in node.children() {
                        stack.push(c);
                    }
                }
            }
        }

        let mut result = Vec::new();
        if let Some(root_id) = self.root_id {
            let mut stack = vec![root_id];
            while let Some(current) = stack.pop() {
                if needed.contains(&current) {
                    result.push(current);
                    if let Some(node) = self.get(current) {
                        for &child_id in node.children().iter().rev() {
                            stack.push(child_id);
                        }
                    }
                }
            }
        }
        result
    }

    pub fn layout(&mut self) {
        let has_valid_root = self
            .root_id
            .and_then(|id| self.get(id))
            .map(|r| r.frame().w > 0.0 && r.frame().h > 0.0)
            .unwrap_or(false);
        if !has_valid_root {
            if let Some(root_id) = self.root_id {
                let ps = self
                    .get(root_id)
                    .map(|r| r.measure(Constraints::unconstrained()));
                if let Some(ps) = ps {
                    if let Some(root_mut) = self.get_mut(root_id) {
                        root_mut.set_frame(Rect::new(0.0, 0.0, ps.w.max(1.0), ps.h.max(1.0)));
                    }
                }
            }
        }

        let mut order = self.layout_traverse();
        if order.is_empty() {
            // 根节点已有有效 frame 但子树尚未布局时（如 resize 后 invalidation 被 reset），
            // 标记 Layout 失效并重新收集遍历顺序，避免组件堆叠在 (0,0)。
            let needs_bootstrap = self.root_id.is_some_and(|root_id| {
                self.get(root_id)
                    .map(|root| {
                        let rf = root.frame();
                        if rf.w <= 0.0 || rf.h <= 0.0 {
                            return true;
                        }
                        root.children().iter().any(|&cid| {
                            self.get(cid).is_some_and(|c| {
                                c.visible()
                                    && c.frame().w <= 0.0
                                    && c.frame().h <= 0.0
                                    && c.measure(Constraints::unconstrained()).h <= 0.0
                            })
                        })
                    })
                    .unwrap_or(false)
            });
            if !needs_bootstrap {
                return;
            }
            if let Some(root_id) = self.root_id {
                self.push_layout_invalidation(root_id);
            }
            order = self.layout_traverse();
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
        for _converge_pass in 0..max_passes {
            let mut any_change = false;

            // Phase 1: Top-down — 父容器根据当前 frame 为子节点分配位置
            let order = self.layout_traverse();
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
                    if let Some(child) = self.get_mut(child_id) {
                        let old = child.frame();
                        if old != rect {
                            self.set_frame_dirty(child_id, rect);
                        }
                    }
                }
            }

            // 预计算逆序遍历顺序，供 Phase 2/4 复用（避免每次 inner pass 重复 clone）
            let rev_order: Vec<WidgetId> = order.iter().rev().copied().collect();

            // 内循环：交替扩展和收缩直到稳定
            for _inner_pass in 0..3 {
                let expanded = self.layout_expand(&rev_order);
                let shrunk = self.layout_shrink(&rev_order);
                if expanded || shrunk {
                    any_change = true;
                }
                if !expanded && !shrunk {
                    break;
                }
            }

            // Phase 3: 更新 viewport 容器的 content_bounds
            self.layout_viewports();

            if !any_change {
                break;
            }
        }

        // 最终更新 viewport（确保收敛结束后的 content_bounds 正确）
        self.layout_viewports();
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
        let entries: Vec<_> = self
            .traverse()
            .into_iter()
            .filter_map(|id| {
                let node = self.get(id)?;
                if !node.visible() {
                    return None;
                }
                node.overlay_entry(id, node.frame())
            })
            .collect();

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
    }

    pub(crate) fn reconcile_lifecycle_after_layout(&mut self) {
        let states: Vec<(WidgetId, bool)> = self
            .traverse()
            .into_iter()
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

    fn visible_rect_for(&self, id: WidgetId) -> Option<Rect> {
        let node = self.get(id)?;
        if !node.visible() {
            return None;
        }
        let mut rect = node.frame();
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return None;
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

    /// 自下而上扩展：当子节点底部超出容器底部时，扩展容器高度。
    /// 后序遍历确保子节点先扩展、父节点后扩展。
    /// 返回是否有任何容器被扩展。
    fn layout_expand(&mut self, rev_order: &[WidgetId]) -> bool {
        let mut any_resized = false;
        // 收集本趟中被扩展过的子节点，用于触发其父容器重排
        let mut resized_children = std::collections::HashSet::new();
        for &id in rev_order {
            let (children, is_viewport, node_frame) = match self.get(id) {
                Some(n) if !n.children().is_empty() => (
                    n.children().to_vec(),
                    n.children_clip(n.frame()).is_some(),
                    n.frame(),
                ),
                _ => continue,
            };
            // Viewport 容器（ScrollView）不扩展，content_bounds 在 layout_viewports 中更新
            if is_viewport {
                continue;
            }

            // 检查是否有直接子节点在本趟中被扩展过
            let has_resized_child = children.iter().any(|cid| resized_children.contains(cid));

            // 取所有可见子节点的最大下边界
            let mut max_bottom = node_frame.y + node_frame.h;
            for &cid in &children {
                if let Some(child) = self.get(cid) {
                    if child.visible() {
                        let cf = child.frame();
                        let child_bottom = cf.y + cf.h;
                        let rel_bottom = (cf.y - node_frame.y) + cf.h;
                        // 只考虑延伸到可见区域的子节点（防止滚动到视口上方时无限膨胀）
                        let child_extends_below_parent = cf.y + cf.h > node_frame.y;
                        if child_bottom > 0.0
                            && child_extends_below_parent
                            && rel_bottom > node_frame.h
                        {
                            max_bottom = max_bottom.max(child_bottom);
                        }
                    }
                }
            }

            let new_h = max_bottom - node_frame.y;
            let needs_relayout = new_h > node_frame.h + 0.5 || has_resized_child;
            if needs_relayout {
                let old_frame = node_frame;
                let effective_h = new_h.max(node_frame.h);
                if effective_h > node_frame.h + 0.5 {
                    crate::core::log::debug_fn(format!(
                        "[Layout] Phase 2: id={} frame_h {:.0} → {:.0} (child bottom={:.0})",
                        id, node_frame.h, effective_h, max_bottom,
                    ));
                    if let Some(_node_mut) = self.get_mut(id) {
                        self.set_frame_dirty(
                            id,
                            Rect::new(old_frame.x, old_frame.y, old_frame.w, effective_h),
                        );
                    }
                } else if has_resized_child {
                    crate::core::log::debug_fn(format!(
                        "[Layout] Phase 2: id={} re-layout siblings (child resized, frame_h={:.0})",
                        id, node_frame.h,
                    ));
                }
                // 重新布局子节点（容器扩展后 or 子节点被扩展过）
                let relayout_frame = if effective_h > node_frame.h + 0.5 {
                    Rect::new(old_frame.x, old_frame.y, old_frame.w, effective_h)
                } else {
                    old_frame
                };
                let new_positions = self
                    .get(id)
                    .map(|n| n.layout_children(relayout_frame, &children, self))
                    .unwrap_or_default();
                for (child_id, rect) in new_positions {
                    if let Some(child) = self.get_mut(child_id) {
                        let old = child.frame();
                        if old != rect {
                            self.set_frame_dirty(child_id, rect);
                        }
                    }
                }
                any_resized = true;
                resized_children.insert(id);
            }
        }
        any_resized
    }

    /// 更新所有 viewport 容器的 content_bounds。
    /// 只触发 content_bounds 副作用，不移动子节点位置。
    fn layout_viewports(&mut self) {
        for &id in &self.layout_traverse() {
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
        use crate::ui::foundation::virtual_scroll::VirtualScroll;

        let ids = self.traverse();
        for id in ids {
            let viewport_h = self
                .get(id)
                .map(|node| node.frame().h)
                .unwrap_or(0.0);
            if viewport_h <= 0.0 {
                continue;
            }
            let needs_refresh = self
                .get(id)
                .and_then(|node| {
                    node.component()
                        .as_any()
                        .downcast_ref::<VirtualScroll>()
                })
                .is_some_and(|vs| vs.needs_child_refresh(viewport_h));
            if !needs_refresh {
                continue;
            }

            let child_nodes = {
                let node = match self.get(id) {
                    Some(node) => node,
                    None => continue,
                };
                let vs = match node.component().as_any().downcast_ref::<VirtualScroll>() {
                    Some(vs) => vs,
                    None => continue,
                };
                vs.build_visible_children(viewport_h)
            };
            self.set_children(id, child_nodes);
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

    /// 收缩过大的容器。与 layout_expand 相反——当子节点高度
    /// 显著小于容器当前高度，且子节点延伸到可见区域时，收缩容器。
    /// 每轮先重新布局子节点（确保兄弟组件靠拢），再检查是否需要收缩。
    /// 返回是否有任何容器被收缩。
    fn layout_shrink(&mut self, rev_order: &[WidgetId]) -> bool {
        let mut any_changed = false;
        for _pass in 0..3 {
            // Phase A: 收集需要收缩的容器
            #[derive(Clone)]
            struct ShrinkOp {
                id: WidgetId,
                needed_h: f32,
            }
            let mut ops: Vec<ShrinkOp> = Vec::new();

            for &id in rev_order {
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
                    if let Some(child) = self.get_mut(child_id) {
                        let old = child.frame();
                        if old != rect {
                            self.set_frame_dirty(child_id, rect);
                            any_changed = true;
                        }
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
                let pref_h = self
                    .get(id)
                    .map(|n| n.measure(Constraints::unconstrained()).h)
                    .unwrap_or(0.0);
                let min_h = needed_h.max(pref_h).max(1.0);
                let effective_needed = min_h;
                if node_frame.h - effective_needed > 0.5 {
                    crate::core::log::debug_fn(format!("[Layout] Phase 4: id={} shrink {:.0}px {:.0}→{:.0} (needed={:.0} pref={:.0})",
                        id, node_frame.h - effective_needed, node_frame.h, effective_needed, needed_h, pref_h,));
                    ops.push(ShrinkOp {
                        id,
                        needed_h: effective_needed,
                    });
                }
            }
            // Phase B: 执行收缩
            for op in &ops {
                if let Some(old_frame) = self.get(op.id).map(|n| n.frame()) {
                    if let Some(_node_mut) = self.get_mut(op.id) {
                        self.set_frame_dirty(
                            op.id,
                            Rect::new(old_frame.x, old_frame.y, old_frame.w, op.needed_h),
                        );
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
                        if let Some(child) = self.get_mut(child_id) {
                            let old = child.frame();
                            if old != rect {
                                self.set_frame_dirty(child_id, rect);
                            }
                        }
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
                                if let Some(child) = self.get_mut(child_id) {
                                    let old = child.frame();
                                    if old != rect {
                                        self.set_frame_dirty(child_id, rect);
                                    }
                                }
                            }
                        }
                    }
                    any_changed = true;
                }
            }
            if !any_changed {
                break;
            }
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
            .into_iter()
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
            updates.push((id, still_active));
        }
        updates
    }
}
