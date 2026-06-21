use super::*;
use crate::base::Rect;
use crate::graphics::DirtyRegion;

/// Widget tree — 管理 BoxedWidget 节点树。
pub struct WidgetTree {
    pub(crate) nodes: Vec<Option<BoxedWidget>>,
    pub(crate) free_ids: Vec<WidgetId>,
    pub(crate) next_id: WidgetId,
    pub(crate) root_id: Option<WidgetId>,
    pub(crate) focused_widget: Option<WidgetId>,
    pub(crate) hovered_widget: Option<WidgetId>,
    pub(crate) dirty_region: DirtyRegion,
    pub(crate) mouse_down_target: Option<WidgetId>,
    pub(crate) scroll_deltas: Vec<(Rect, f32, f32)>,
    /// 树结构版本号，结构变更时递增（add_child / remove / set_root）。
    /// 引擎可用此判断 LayerTree 是否需要重建。
    pub(crate) tree_version: u64,
    /// 缓存的先序遍历结果（内部可变性，仅用作性能缓存）。
    /// 当 `cached_traversal_version != tree_version` 时失效重建。
    cached_traversal: std::cell::RefCell<(Vec<WidgetId>, u64)>,
}

impl Default for WidgetTree {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            free_ids: Vec::new(),
            next_id: 0,
            root_id: None,
            focused_widget: None,
            hovered_widget: None,
            dirty_region: DirtyRegion::full(),
            mouse_down_target: None,
            scroll_deltas: Vec::new(),
            tree_version: 0,
            cached_traversal: std::cell::RefCell::new((Vec::new(), 0)),
        }
    }
}

impl WidgetTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回当前树结构版本号。结构变更（add_child / remove / set_root）时递增。
    pub fn tree_version(&self) -> u64 {
        self.tree_version
    }

    pub fn alloc_id(&mut self) -> WidgetId {
        if let Some(id) = self.free_ids.pop() {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn set_root_with_children(
        &mut self,
        widget: Box<dyn Widget>,
        children: Vec<Box<dyn Widget>>,
    ) -> WidgetId {
        let id = self.set_root(widget);
        for child in children {
            self.add_child(id, child);
        }
        id
    }

    pub fn add_child_with_children(
        &mut self,
        parent_id: WidgetId,
        widget: Box<dyn Widget>,
        children: Vec<Box<dyn Widget>>,
    ) -> WidgetId {
        let id = self.add_child(parent_id, widget);
        for child in children {
            self.add_child(id, child);
        }
        id
    }

    /// 重置所有指向旧 widget ID 的交互状态（树重建时使用）。
    fn reset_interaction_state(&mut self) {
        self.focused_widget = None;
        self.hovered_widget = None;
        self.mouse_down_target = None;
        self.scroll_deltas.clear();
    }

    pub fn set_root(&mut self, widget: Box<dyn Widget>) -> WidgetId {
        self.tree_version += 1;
        self.reset_interaction_state();
        let children = widget.build();
        let id = self.alloc_id();
        let mut boxed = BoxedWidget::new(widget);
        boxed.set_id(id);
        let ps = boxed.preferred_size(None);
        boxed.set_frame(Rect::new(0.0, 0.0, ps.w, ps.h));
        if self.nodes.len() <= id {
            self.nodes.resize_with(id + 1, || None);
        }
        self.nodes[id] = Some(boxed);
        self.root_id = Some(id);
        for child in children {
            self.add_child(id, child);
        }
        id
    }

    pub fn root(&self) -> Option<&BoxedWidget> {
        self.root_id
            .and_then(|id| self.nodes.get(id))
            .and_then(|n| n.as_ref())
    }
    pub fn root_id(&self) -> Option<WidgetId> {
        self.root_id
    }
    pub fn root_mut(&mut self) -> Option<&mut BoxedWidget> {
        self.root_id
            .and_then(|id| self.nodes.get_mut(id))
            .and_then(|n| n.as_mut())
    }

    pub fn find_by_type<T: Widget + 'static>(&self) -> Option<WidgetId> {
        for id in self.traverse() {
            if let Some(node) = self.get(id) {
                if node.inner().as_any().downcast_ref::<T>().is_some() {
                    return Some(id);
                }
            }
        }
        None
    }

    pub fn find_all_by_type<T: Widget + 'static>(&self) -> Vec<(WidgetId, &T)> {
        let mut results = Vec::new();
        for id in self.traverse() {
            if let Some(node) = self.get(id) {
                if let Some(w) = node.inner().as_any().downcast_ref::<T>() {
                    results.push((id, w));
                }
            }
        }
        results
    }

    pub fn find_by_type_and_modify<T: Widget + 'static>(
        &mut self,
        f: impl FnOnce(&mut T),
    ) -> Option<WidgetId> {
        let id = self.find_by_type::<T>()?;
        if let Some(node) = self.get_mut(id) {
            if let Some(w) = node.inner_mut().as_any_mut().downcast_mut::<T>() {
                f(w);
            }
        }
        Some(id)
    }

    pub fn get(&self, id: WidgetId) -> Option<&BoxedWidget> {
        self.nodes.get(id).and_then(|n| n.as_ref())
    }
    pub fn get_mut(&mut self, id: WidgetId) -> Option<&mut BoxedWidget> {
        self.nodes.get_mut(id).and_then(|n| n.as_mut())
    }

    pub fn set_z_index(&mut self, id: WidgetId, z: i32) -> &mut Self {
        if let Some(n) = self.get_mut(id) {
            n.set_z_index(z);
        }
        self
    }

    pub fn add_child(&mut self, parent_id: WidgetId, child: Box<dyn Widget>) -> WidgetId {
        self.tree_version += 1;
        let children = child.build();
        let child_id = self.alloc_id();
        let mut boxed = BoxedWidget::new(child);
        boxed.set_id(child_id);
        boxed.set_parent(Some(parent_id));
        if self.nodes.len() <= child_id {
            self.nodes.resize_with(child_id + 1, || None);
        }
        self.nodes[child_id] = Some(boxed);
        if let Some(parent) = self.get_mut(parent_id) {
            parent.children_mut().push(child_id);
        }
        for child in children {
            self.add_child(child_id, child);
        }
        child_id
    }

    pub fn remove(&mut self, id: WidgetId) {
        self.tree_version += 1;
        let parent_id = self
            .nodes
            .get(id)
            .and_then(|n| n.as_ref())
            .and_then(|n| n.parent());
        if let Some(node) = self.nodes.get_mut(id) {
            if let Some(node) = node.take() {
                for child_id in node.children().to_vec() {
                    self.remove(child_id);
                }
                self.free_ids.push(id);
            }
        }
        if let Some(pid) = parent_id {
            if let Some(parent) = self.get_mut(pid) {
                parent.children_mut().retain(|&c| c != id);
            }
        }
    }

    /// 返回树中所有节点的先序遍历顺序。
    ///
    /// 内部使用缓存：当树结构未变化时克隆缓存结果（O(n) memcpy），
    /// 避免每帧多次完整遍历 + Vec 分配的开销。
    pub fn traverse(&self) -> Vec<WidgetId> {
        let mut cache = self.cached_traversal.borrow_mut();
        let (ref mut ids, ref mut ver) = *cache;
        if *ver != self.tree_version {
            ids.clear();
            if let Some(root_id) = self.root_id {
                // 迭代遍历（避免递归过深时的栈溢出）
                let mut stack = vec![root_id];
                while let Some(current) = stack.pop() {
                    ids.push(current);
                    if let Some(node) = self.get(current) {
                        for child_id in node.children().iter().rev() {
                            stack.push(*child_id);
                        }
                    }
                }
            }
            *ver = self.tree_version;
        }
        ids.clone()
    }

    pub fn layout(&mut self) {
        let has_valid_root = self
            .root_id
            .and_then(|id| self.get(id))
            .map(|r| r.frame().w > 0.0 && r.frame().h > 0.0)
            .unwrap_or(false);
        if !has_valid_root {
            if let Some(root_id) = self.root_id {
                let ps = self.get(root_id).map(|r| r.preferred_size(None));
                if let Some(ps) = ps {
                    if let Some(root_mut) = self.get_mut(root_id) {
                        root_mut.set_frame(Rect::new(0.0, 0.0, ps.w.max(1.0), ps.h.max(1.0)));
                    }
                }
            }
        }

        // ════════════════════════════════════════════════════════════════
        // 收敛循环：自上而下布局 → [扩展 ↔ 收缩] → viewport
        // Phase 2（扩展）和 Phase 4（收缩）交替运行直至稳定，
        // 防止两个阶段的尺寸调整形成逐帧振荡。
        // ════════════════════════════════════════════════════════════════
        for _converge_pass in 0..5 {
            let mut any_change = false;

            // Phase 1: Top-down — 父容器根据当前 frame 为子节点分配位置
            let order = self.traverse();
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
                    node.inner().layout_children(frame, &children, self)
                };
                for (child_id, rect) in positions {
                    if let Some(child) = self.get_mut(child_id) {
                        let old = child.frame();
                        if old != rect {
                            child.set_frame(rect);
                            self.mark_dirty_rect(child_id, old);
                            self.mark_dirty(child_id);
                        }
                    }
                }
            }

            // 内循环：交替扩展和收缩直到稳定
            for _inner_pass in 0..3 {
                let expanded = self.layout_expand();
                let shrunk = self.layout_shrink();
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
        log::debug!("[Layout] layout() done");
    }

    /// 自下而上扩展：当子节点底部超出容器底部时，扩展容器高度。
    /// 后序遍历确保子节点先扩展、父节点后扩展。
    /// 返回是否有任何容器被扩展。
    fn layout_expand(&mut self) -> bool {
        let mut any_resized = false;
        let rev_order: Vec<WidgetId> = self.traverse().into_iter().rev().collect();
        for &id in &rev_order {
            let (children, is_viewport, node_frame) = match self.get(id) {
                Some(n) if !n.children().is_empty() => {
                    (n.children().to_vec(), n.inner().children_clip(n.frame()).is_some(), n.frame())
                }
                _ => continue,
            };
            // Viewport 容器（ScrollView）不扩展，content_bounds 在 layout_viewports 中更新
            if is_viewport {
                continue;
            }

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
                        if child_bottom > 0.0 && child_extends_below_parent && rel_bottom > node_frame.h {
                            max_bottom = max_bottom.max(child_bottom);
                        }
                    }
                }
            }

            let new_h = max_bottom - node_frame.y;
            if new_h > node_frame.h + 0.5 {
                log::debug!(
                    "[Layout] Phase 2: id={} frame_h {:.0} → {:.0} (child bottom={:.0})",
                    id, node_frame.h, new_h, max_bottom,
                );
                let old_frame = node_frame;
                if let Some(node_mut) = self.get_mut(id) {
                    node_mut.set_frame(Rect::new(old_frame.x, old_frame.y, old_frame.w, new_h));
                    self.mark_dirty_rect(id, old_frame);
                    self.mark_dirty(id);
                }
                // 容器扩展后，重新布局子节点
                let new_frame = Rect::new(old_frame.x, old_frame.y, old_frame.w, new_h);
                let new_positions = self
                    .get(id)
                    .map(|n| n.inner().layout_children(new_frame, &children, self))
                    .unwrap_or_default();
                for (child_id, rect) in new_positions {
                    if let Some(child) = self.get_mut(child_id) {
                        let old = child.frame();
                        if old != rect {
                            child.set_frame(rect);
                            self.mark_dirty_rect(child_id, old);
                            self.mark_dirty(child_id);
                        }
                    }
                }
                any_resized = true;
            }
        }
        any_resized
    }

    /// 更新所有 viewport 容器的 content_bounds。
    /// 只触发 content_bounds 副作用，不移动子节点位置。
    fn layout_viewports(&mut self) {
        for &id in &self.traverse() {
            if let Some(node) = self.get(id) {
                if node.inner().children_clip(node.frame()).is_none() {
                    continue;
                }
                let frame = node.frame();
                let children = node.children().to_vec();
                if children.is_empty() {
                    continue;
                }
                log::debug!(
                    "[Layout] Phase 3: viewport id={} frame=({:.0},{:.0},{:.0},{:.0}) {} children",
                    id, frame.x, frame.y, frame.w, frame.h, children.len(),
                );
                // 仅触发 content_bounds 副作用，丢弃返回的 child rects
                let _ = node.inner().layout_children(frame, &children, self);
            }
        }
    }

    /// 收缩过大的容器。与 layout_expand 相反——当子节点高度
    /// 显著小于容器当前高度，且子节点延伸到可见区域时，收缩容器。
    /// 每轮先重新布局子节点（确保兄弟组件靠拢），再检查是否需要收缩。
    /// 返回是否有任何容器被收缩。
    fn layout_shrink(&mut self) -> bool {
        let mut any_changed = false;
        for _pass in 0..3 {
            let rev_order: Vec<WidgetId> = self.traverse().into_iter().rev().collect();
            // Phase A: 收集需要收缩的容器
            #[derive(Clone)]
            struct ShrinkOp { id: WidgetId, needed_h: f32 }
            let mut ops: Vec<ShrinkOp> = Vec::new();

            for &id in &rev_order {
                let is_viewport = self.get(id)
                    .map(|n| n.inner().children_clip(n.frame()).is_some())
                    .unwrap_or(false);
                if is_viewport { continue; }
                // 不收缩父容器是 viewport（如 ScrollView）的子节点，
                // 避免与 ScrollView::layout_children 的尺寸设定形成振荡。
                let parent_is_viewport = self.get(id)
                    .and_then(|n| n.parent())
                    .and_then(|pid| self.get(pid))
                    .map(|p| p.inner().children_clip(p.frame()).is_some())
                    .unwrap_or(false);
                if parent_is_viewport { continue; }
                // 也不收缩祖先链上任一节点是 viewport 的容器（深层嵌套保护）
                let mut ancestor_is_viewport = false;
                let mut cur = self.get(id).and_then(|n| n.parent());
                while let Some(pid) = cur {
                    if let Some(p) = self.get(pid) {
                        if p.inner().children_clip(p.frame()).is_some() {
                            ancestor_is_viewport = true;
                            break;
                        }
                        cur = p.parent();
                    } else {
                        break;
                    }
                }
                if ancestor_is_viewport { continue; }

                let children: Vec<WidgetId> = match self.get(id) {
                    Some(n) if !n.children().is_empty() => n.children().to_vec(),
                    _ => continue,
                };

                // 先按当前 frame 重新布局子节点（兄弟组件靠拢/张开）
                let Some(frame) = self.get(id).map(|n| n.frame()) else { continue; };
                let positions: Vec<(WidgetId, Rect)> = {
                    let Some(node) = self.get(id) else { continue; };
                    node.inner().layout_children(frame, &children, self)
                };
                for (child_id, rect) in positions {
                    if let Some(child) = self.get_mut(child_id) {
                        let old = child.frame();
                        if old != rect {
                            child.set_frame(rect);
                            self.mark_dirty_rect(child_id, old);
                            self.mark_dirty(child_id);
                            any_changed = true;
                        }
                    }
                }

                // 检查容器是否需要收缩
                let Some(node_frame) = self.get(id).map(|n| n.frame()) else { continue; };
                let mut max_child_bottom = f32::MIN;
                let mut has_visible = false;
                for &cid in &children {
                    if let Some(child) = self.get(cid) {
                        if child.visible()
                        {
                            let cf = child.frame();
                            let child_bottom = cf.y + cf.h;
                            if child_bottom > 0.0 {
                                max_child_bottom = max_child_bottom.max(child_bottom);
                                has_visible = true;
                            }
                        }
                    }
                }
                if !has_visible { continue; }

                let needed_h = max_child_bottom - node_frame.y;
                // ⭐ 最小高度取子节点实际内容和 preferred_size 的较大值。
                // 设此下限可防止收缩到子节点内容以下，从而避免与
                // layout_expand（Phase 2）形成振荡循环。
                let pref_h = self.get(id)
                    .map(|n| n.preferred_size(None).h)
                    .unwrap_or(0.0);
                let min_h = needed_h.max(pref_h).max(node_frame.h * 0.01);
                let effective_needed = min_h;
                if node_frame.h - effective_needed > 5.0 {
                    log::debug!(
                        "[Layout] Phase 4: id={} shrink {:.0}px {:.0}→{:.0} (needed={:.0} pref={:.0})",
                        id, node_frame.h - effective_needed, node_frame.h, effective_needed, needed_h, pref_h,
                    );
                    ops.push(ShrinkOp { id, needed_h: effective_needed });
                }
            }
            // Phase B: 执行收缩
            for op in &ops {
                if let Some(old_frame) = self.get(op.id).map(|n| n.frame()) {
                    if let Some(node_mut) = self.get_mut(op.id) {
                        node_mut.set_frame(Rect::new(old_frame.x, old_frame.y, old_frame.w, op.needed_h));
                        self.mark_dirty_rect(op.id, old_frame);
                        self.mark_dirty(op.id);
                    }
                    let children: Vec<WidgetId> = self.get(op.id)
                        .map(|n| n.children().to_vec())
                        .unwrap_or_default();
                    let new_frame = Rect::new(old_frame.x, old_frame.y, old_frame.w, op.needed_h);
                    // 收缩后重新布局子节点
                    let new_positions = self.get(op.id)
                        .map(|n| n.inner().layout_children(new_frame, &children, self))
                        .unwrap_or_default();
                    for (child_id, rect) in new_positions {
                        if let Some(child) = self.get_mut(child_id) {
                            let old = child.frame();
                            if old != rect {
                                child.set_frame(rect);
                                self.mark_dirty_rect(child_id, old);
                                self.mark_dirty(child_id);
                            }
                        }
                    }
                    // 重新布局父容器，让兄弟组件靠拢
                    if let Some(pid) = self.get(op.id).and_then(|n| n.parent()) {
                        let parent_frame = self.get(pid).map(|n| n.frame()).unwrap_or_default();
                        let parent_children: Vec<WidgetId> = self.get(pid)
                            .map(|n| n.children().to_vec())
                            .unwrap_or_default();
                        if !parent_children.is_empty() {
                            let parent_positions = self.get(pid)
                                .map(|n| n.inner().layout_children(parent_frame, &parent_children, self))
                                .unwrap_or_default();
                            for (child_id, rect) in parent_positions {
                                if let Some(child) = self.get_mut(child_id) {
                                    let old = child.frame();
                                    if old != rect {
                                        child.set_frame(rect);
                                        self.mark_dirty_rect(child_id, old);
                                        self.mark_dirty(child_id);
                                    }
                                }
                            }
                        }
                    }
                    any_changed = true;
                }
            }
            if !any_changed { break; }
        }
        any_changed
    }

    pub fn update(&mut self, dt: f32) -> bool {
        let order = self.traverse();
        let mut any_animating = false;
        for &id in &order {
            let was_animating = self
                .get(id)
                .map(|n| n.inner().needs_continuous_update())
                .unwrap_or(false);
            if let Some(node) = self.get_mut(id) {
                node.inner_mut().on_update(dt);
            }
            let (rect, scroll) = self
                .get(id)
                .map(|node| {
                    let is_still = node.inner().needs_continuous_update();
                    let dirty = if was_animating || is_still {
                        any_animating = any_animating || is_still;
                        node.inner().dirty_rect(node.frame())
                    } else {
                        Rect::zero()
                    };
                    (dirty, node.inner().scroll_delta(node.frame()))
                })
                .unwrap_or_default();
            if rect.w > 0.0 || rect.h > 0.0 {
                self.mark_dirty_rect(id, rect);
            }
            if let Some((dx, dy)) = scroll {
                if dx != 0.0 || dy != 0.0 {
                    let frame = self.get(id).map(|n| n.frame()).unwrap_or_default();
                    self.scroll_deltas.push((frame, dx, dy));
                }
            }
        }
        any_animating
    }

    // ── WidgetNode tree building ──

    /// 从根节点构建整棵树。总是分配新的 widget_id。
    pub fn build(&mut self, node: WidgetNode) -> WidgetId {
        self.build_node(node, None)
    }

    /// 替换指定节点的所有子节点为新子树。
    /// 父节点 widget_id 不变（保持 LayerTree 缓存），子节点分配新 ID。
    /// 适合页面切换等局部更新的场景。
    pub fn set_children(&mut self, parent_id: WidgetId, children: Vec<WidgetNode>) {
        let old_children: Vec<WidgetId> = self.get(parent_id)
            .map(|n| n.children().to_vec())
            .unwrap_or_default();
        for &cid in &old_children {
            self.remove(cid);
        }
        self.tree_version += 1;
        for child in children {
            self.build_node(child, Some(parent_id));
        }
    }

    /// 递归构建节点及其子树。
    fn build_node(&mut self, node: WidgetNode, parent: Option<WidgetId>) -> WidgetId {
        let id = match parent {
            Some(p) => self.add_child(p, node.widget),
            None => self.set_root(node.widget),
        };
        if let Some(n) = self.get_mut(id) {
            n.set_z_index(node.z_index);
        }
        for child in node.children {
            self.build_node(child, Some(id));
        }
        id
    }

    /// 收缩 nodes Vec 的容量以适应当前活跃节点数。
    /// 删除节点后调用可释放空闲插槽占用的内存。
    pub fn shrink_to_fit(&mut self) {
        self.nodes.shrink_to_fit();
    }

    /// 释放 tree 内部所有 Vec 的额外容量。
    pub fn shrink_all(&mut self) {
        self.nodes.shrink_to_fit();
        self.free_ids.shrink_to_fit();
        self.scroll_deltas.shrink_to_fit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Point;
    use crate::base::KeyMod;
    use std::cell::RefCell;

    struct SpyWidget {
        size: crate::base::Size,
        last_event: RefCell<Option<WidgetEvent>>,
    }
    impl SpyWidget {
        fn new(w: f32, h: f32) -> Self {
            Self {
                size: crate::base::Size::new(w, h),
                last_event: RefCell::new(None),
            }
        }
    }
    impl Widget for SpyWidget {
        fn preferred_size(
            &self,
            _: Option<&dyn crate::graphics::GraphicsEngine>,
        ) -> crate::base::Size {
            self.size
        }
        fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
            *self.last_event.borrow_mut() = Some(event.clone());
            EventResult::Handled
        }
        fn render(
            &self,
            _: Rect,
            _: &mut crate::ui::render_context::RenderContext,
            _: &WidgetTree,
        ) {
        }
    }

    struct PassThroughContainer {
        size: crate::base::Size,
        children: RefCell<Vec<Box<dyn Widget>>>,
    }
    impl PassThroughContainer {
        fn new(w: f32, h: f32, children: Vec<Box<dyn Widget>>) -> Self {
            Self {
                size: crate::base::Size::new(w, h),
                children: RefCell::new(children),
            }
        }
    }
    impl Widget for PassThroughContainer {
        fn build(&self) -> Vec<Box<dyn Widget>> {
            std::mem::take(&mut *self.children.borrow_mut())
        }
        fn preferred_size(
            &self,
            _: Option<&dyn crate::graphics::GraphicsEngine>,
        ) -> crate::base::Size {
            self.size
        }
        fn on_event(&mut self, _: &WidgetEvent) -> EventResult {
            EventResult::NotHandled
        }
        fn render(
            &self,
            _: Rect,
            _: &mut crate::ui::render_context::RenderContext,
            _: &WidgetTree,
        ) {
        }
    }

    #[test]
    fn tree_set_root_returns_valid_id() {
        let mut tree = WidgetTree::new();
        let id = tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
        assert!(tree.get(id).is_some());
        assert_eq!(tree.root().unwrap().id(), id);
    }

    #[test]
    fn tree_add_child_links_parent() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
        let cid = tree.add_child(root, Box::new(SpyWidget::new(80.0, 40.0)));
        assert!(tree.get(cid).is_some());
        assert_eq!(tree.get(root).unwrap().children(), &[cid]);
        assert_eq!(tree.get(cid).unwrap().parent(), Some(root));
    }

    #[test]
    fn tree_traverse_preorder() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
        let a = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
        let b = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
        let c = tree.add_child(a, Box::new(SpyWidget::new(25.0, 15.0)));
        assert_eq!(tree.traverse(), vec![root, a, c, b]);
    }

    #[test]
    fn tree_remove_cascades_to_children() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
        let a = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
        let b = tree.add_child(a, Box::new(SpyWidget::new(25.0, 15.0)));
        tree.remove(a);
        assert!(tree.get(a).is_none());
        assert!(tree.get(b).is_none());
        assert_eq!(tree.get(root).unwrap().children().len(), 0);
    }

    #[test]
    fn hit_test_root_contains() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
        tree.layout();
        assert!(tree.hit_test(Point::new(50.0, 25.0)).is_some());
    }

    #[test]
    fn hit_test_outside_returns_none() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
        tree.layout();
        assert!(tree.hit_test(Point::new(200.0, 200.0)).is_none());
        assert!(tree.hit_test(Point::new(-1.0, 25.0)).is_none());
    }

    #[test]
    fn hit_test_returns_deepest_child() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
        let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
        tree.get_mut(root)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(child)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(child));
    }

    #[test]
    fn hit_test_skips_invisible() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
        let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
        tree.get_mut(root)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(child)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        tree.get_mut(child).unwrap().set_visible(false);
        assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(root));
    }

    #[test]
    fn dispatch_mouse_down_focuses_target() {
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
        let btn = tree.add_child(root_id, Box::new(SpyWidget::new(80.0, 40.0)));
        tree.get_mut(root_id)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(btn)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 80.0, 40.0));
        assert_eq!(
            tree.dispatch_event(&WidgetEvent::MouseDown {
                pos: Point::new(40.0, 20.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            }),
            EventResult::Handled
        );
    }

    #[test]
    fn dispatch_mouse_down_empty_space_clears_focus() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
        tree.layout();
        tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(300.0, 300.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        });
    }

    #[test]
    fn dispatch_key_to_focused_widget() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
        tree.layout();
        tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 50.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        });
        assert_eq!(
            tree.dispatch_event(&WidgetEvent::KeyDown {
                key: KeyCode::Enter,
                mods: KeyMod::NONE,
            }),
            EventResult::Handled
        );
    }

    #[test]
    fn dispatch_mouse_move_triggers_hover_enter_leave() {
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
        let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
        tree.get_mut(root_id)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(child)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        tree.dispatch_event(&WidgetEvent::MouseMove {
            pos: Point::new(50.0, 50.0),
        });
    }

    #[test]
    fn dispatch_resize_goes_to_root() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
        tree.layout();
        assert_eq!(
            tree.dispatch_event(&WidgetEvent::Resize {
                width: 400.0,
                height: 300.0
            }),
            EventResult::Handled
        );
    }

    #[test]
    fn nav_item_click_updates_shared_active() {
        use crate::ui::widgets::nav::{NavItem, SharedActive};
        use std::cell::Cell;
        use std::rc::Rc;
        let active: SharedActive = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(
            crate::ui::widgets::Container::new()
                .size(200.0, 200.0)
                .dir(crate::ui::FlexDirection::Column),
        ));
        let n0 = tree.add_child(
            root,
            Box::new(
                NavItem::new("Item 0", 0, active.clone())
                    .width(200.0)
                    .height(36.0),
            ),
        );
        let n1 = tree.add_child(
            root,
            Box::new(
                NavItem::new("Item 1", 1, active.clone())
                    .width(200.0)
                    .height(36.0),
            ),
        );
        tree.get_mut(root)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(n0)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 200.0, 36.0));
        tree.get_mut(n1)
            .unwrap()
            .set_frame(Rect::new(0.0, 36.0, 200.0, 36.0));
        assert_eq!(active.get(), 0);
        let result = tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 54.0),
            button: crate::base::MouseButton::Left,
            mods: KeyMod::NONE,
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(active.get(), 1);
        let result = tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 18.0),
            button: crate::base::MouseButton::Left,
            mods: KeyMod::NONE,
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(active.get(), 0);
        let result = tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 150.0),
            button: crate::base::MouseButton::Left,
            mods: KeyMod::NONE,
        });
        assert_eq!(result, EventResult::NotHandled);
        assert_eq!(active.get(), 0);
    }
}
