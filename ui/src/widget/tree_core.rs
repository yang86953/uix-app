use super::*;
use uix_platform::Rect;
use uix_graphics::DirtyRegion;

/// 脏状态管理器 —— 集中管理脏区域和滚动数据。
///
/// 保证 reset() 时不会遗漏任何需要清理的脏状态。
pub struct DirtyState {
    pub(crate) region: DirtyRegion,
    pub(crate) scroll_deltas: Vec<(Rect, f32, f32)>,
}

impl DirtyState {
    pub fn new() -> Self {
        Self {
            region: DirtyRegion::full(),
            scroll_deltas: Vec::new(),
        }
    }

    /// 重置所有脏状态。
    pub fn reset(&mut self) {
        self.region.reset();
        self.scroll_deltas.clear();
    }
}

impl Default for DirtyState {
    fn default() -> Self {
        Self::new()
    }
}

/// BitSet 脏节点标记 —— 用 trailing_zeros 跳过零位，
/// 遍历复杂度与脏节点数成正比，与总节点数无关。
pub struct DirtyNodes {
    words: Vec<u64>,
}

impl DirtyNodes {
    pub fn new() -> Self {
        Self { words: Vec::new() }
    }

    /// 标记节点为脏。自动扩容 bitset。
    pub fn insert(&mut self, id: WidgetId) {
        let idx = id / 64;
        let bit = 1 << (id % 64);
        if idx >= self.words.len() {
            self.words.resize(idx + 1, 0);
        }
        self.words[idx] |= bit;
    }

    /// 清零所有脏标记（O(n/64)，SIMD 友好）。
    pub fn clear(&mut self) {
        self.words.iter_mut().for_each(|w| *w = 0);
    }

    /// 是否有任何脏节点。
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    /// 判断节点是否在 bitset 中。
    pub fn contains(&self, id: WidgetId) -> bool {
        let idx = id / 64;
        let bit = 1 << (id % 64);
        self.words.get(idx).is_some_and(|w| w & bit != 0)
    }

    /// 遍历所有脏节点 —— 只迭代被置位的 bit，跳过零。
    pub fn iter_dirty(&self) -> DirtyIter<'_> {
        DirtyIter {
            words: &self.words,
            word_idx: 0,
            current_word: self.words.first().copied().unwrap_or(0),
        }
    }
}

impl Default for DirtyNodes {
    fn default() -> Self {
        Self::new()
    }
}

/// DirtyNodes 的迭代器 —— 每次调用 next() 跳过 64 个干净节点。
pub struct DirtyIter<'a> {
    words: &'a [u64],
    word_idx: usize,
    current_word: u64,
}

impl Iterator for DirtyIter<'_> {
    type Item = WidgetId;

    fn next(&mut self) -> Option<WidgetId> {
        loop {
            if self.current_word != 0 {
                // trailing_zeros: 一条 CPU 指令找到最低位脏节点
                let t = self.current_word.trailing_zeros();
                let id = self.word_idx * 64 + t as usize;
                // blsr: 清除最低位 (x86 BMI1 指令)
                self.current_word &= self.current_word - 1;
                return Some(id);
            }
            self.word_idx += 1;
            if self.word_idx >= self.words.len() {
                return None;
            }
            self.current_word = self.words[self.word_idx];
        }
    }
}

/// Widget tree — 管理 BoxedWidget 节点树。
pub struct WidgetTree {
    pub(crate) nodes: Vec<Option<BoxedWidget>>,
    pub(crate) free_ids: Vec<WidgetId>,
    pub(crate) next_id: WidgetId,
    pub(crate) root_id: Option<WidgetId>,
    pub(crate) focused_widget: Option<WidgetId>,
    pub(crate) hovered_widget: Option<WidgetId>,
    pub(crate) dirty: DirtyState,
    pub(crate) dirty_nodes: DirtyNodes,
    /// 子树脏汇总（终极方案：向上传播的脏标记）。
    /// subtree_dirty.contains(id) = true 表示 id 的子树中有脏节点。
    /// 用于遍历时跳过整棵干净子树。
    pub(crate) subtree_dirty: DirtyNodes,
    pub(crate) mouse_down_target: Option<WidgetId>,
    /// 树结构版本号，结构变更时递增（add_child / remove / set_root）。
    /// 引擎可用此判断 LayerTree 是否需要重建。
    pub tree_version: u64,
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
            dirty: DirtyState::new(),
            dirty_nodes: DirtyNodes::new(),
            subtree_dirty: DirtyNodes::new(),
            mouse_down_target: None,
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
        widget: Box<dyn WidgetComponent>,
        children: Vec<Box<dyn WidgetComponent>>,
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
        widget: Box<dyn WidgetComponent>,
        children: Vec<Box<dyn WidgetComponent>>,
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
        self.dirty.scroll_deltas.clear();
    }

    /// 设置根节点（全量重建）。
    ///
    /// 每次调用会**彻底清空旧树**，ID 空间从 0 重新开始分配。
    /// 这意味着同一棵 widget 树（相同构建顺序）每次重建后拿到相同的 ID。
    pub fn set_root(&mut self, widget: Box<dyn WidgetComponent>) -> WidgetId {
        // 硬重置：清空旧树，ID 空间归零，free_ids 废弃
        self.nodes.clear();
        self.free_ids.clear();
        self.next_id = 0;
        self.root_id = None;
        self.reset_interaction_state();
        self.tree_version += 1;

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

    pub fn find_by_type<T: WidgetComponent + 'static>(&self) -> Option<WidgetId> {
        for id in self.traverse() {
            if let Some(node) = self.get(id) {
                if node.component().as_any().downcast_ref::<T>().is_some() {
                    return Some(id);
                }
            }
        }
        None
    }

    pub fn find_all_by_type<T: WidgetComponent + 'static>(&self) -> Vec<(WidgetId, &T)> {
        let mut results = Vec::new();
        for id in self.traverse() {
            if let Some(node) = self.get(id) {
                if let Some(w) = node.component().as_any().downcast_ref::<T>() {
                    results.push((id, w));
                }
            }
        }
        results
    }

    pub fn find_by_type_and_modify<T: WidgetComponent + 'static>(
        &mut self,
        f: impl FnOnce(&mut T),
    ) -> Option<WidgetId> {
        let id = self.find_by_type::<T>()?;
        if let Some(node) = self.get_mut(id) {
            if let Some(w) = node.component_mut().as_any_mut().downcast_mut::<T>() {
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

    pub fn add_child(&mut self, parent_id: WidgetId, child: Box<dyn WidgetComponent>) -> WidgetId {
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

        // 终极方案：向父链传播子树脏标记（新节点需要加入脏子树遍历）
        self.propagate_subtree_dirty(parent_id);

        child_id
    }

    pub fn remove(&mut self, id: WidgetId) {
        self.tree_version += 1;

        // 在移除前标记旧 frame 为脏，确保该区域被重绘（清除视觉残留）
        let old_frame = self.get(id).map(|n| n.frame()).filter(|f| f.w > 0.0 && f.h > 0.0);

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

        if let Some(frame) = old_frame {
            self.dirty.region.add_rect(frame);
        }

        // 终极方案：向父链传播子树脏标记（结构变化影响父布局）
        if let Some(pid) = parent_id {
            self.propagate_subtree_dirty(pid);
        }
    }

    /// 设置节点可见性并递增 tree_version。
    ///
    /// 可见性变化会改变 LayerTree 结构（不可见节点被排除），
    /// 因此必须通知渲染管线在下帧重建 LayerTree。
    pub fn set_visible(&mut self, id: WidgetId, visible: bool) {
        // 递归设置节点及其所有后代的可见性
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            let children: Vec<WidgetId> = self.get(current)
                .map(|n| n.children().to_vec())
                .unwrap_or_default();

            // 先记录 visible 是否变化（get_mut 的借用释放后再标记 dirty）
            let mut changed = false;
            if let Some(n) = self.get_mut(current) {
                if n.visible() != visible {
                    n.set_visible(visible);
                    n.set_dirty(true);
                    self.tree_version += 1;
                    changed = true;
                }
            }

            // get_mut 的借用已释放，可安全访问 dirty.region 和 dirty_nodes
            if changed {
                self.dirty_nodes.insert(current);
                if let Some(frame) = self.get(current).map(|n| n.frame()) {
                    if frame.w > 0.0 && frame.h > 0.0 {
                        self.dirty.region.add_rect(frame);
                    }
                }
            }

            for child in children {
                stack.push(child);
            }
        }

        // 终极方案：向父链传播子树脏标记（可见性变化影响父布局）
        self.propagate_subtree_dirty(id);
    }

    /// 返回脏子树中所有节点的先序遍历顺序。
    ///
    /// 当 full_frame 时回退到全树 traverse()（初始帧 / 显式全帧标记）。
    /// 其余情况只遍历 subtree_dirty 标记的子树，跳过整棵干净区域。
    /// 遍历复杂度 O(m)，m = 脏子树节点数（通常 << n）。
    pub fn dirty_traverse(&self) -> Vec<WidgetId> {
        if self.dirty.region.full_frame {
            // 全帧脏 → 回退全树遍历（初始帧 / mark_full_frame_dirty）
            return self.traverse();
        }
        let mut result = Vec::new();
        if let Some(root_id) = self.root_id {
            let mut stack = vec![root_id];
            while let Some(current) = stack.pop() {
                result.push(current);
                if let Some(node) = self.get(current) {
                    for &child_id in node.children().iter().rev() {
                        // 子树干净 → 整棵跳过
                        if self.subtree_dirty.contains(child_id) || self.dirty_nodes.contains(child_id) {
                            stack.push(child_id);
                        }
                    }
                }
            }
        }
        result
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

    /// 设置 widget 的 frame 并自动标记旧区域为脏。
    /// 封装了 set_frame + mark_dirty_rect(old) + mark_dirty 的三重模式。
    pub fn set_frame_dirty(&mut self, id: WidgetId, new_frame: Rect) {
        let old = match self.get(id) {
            Some(w) => {
                let old = w.frame();
                if old == new_frame { return; }
                old
            }
            None => return,
        };
        if let Some(w) = self.get_mut(id) {
            w.set_frame(new_frame);
        }
        self.mark_dirty_rect(id, old);
        self.mark_dirty(id);
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
        // 上限提升至 10 次，应对深层嵌套（Container→Container→Widget）场景。
        // ════════════════════════════════════════════════════════════════
        let max_passes = 10;
        for _converge_pass in 0..max_passes {
            let mut any_change = false;

            // Phase 1: Top-down — 父容器根据当前 frame 为子节点分配位置
            let order = self.dirty_traverse();
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
        uix_platform::log::debug_fn("[Layout] layout() done");
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
                Some(n) if !n.children().is_empty() => {
                    (n.children().to_vec(), n.children_clip(n.frame()).is_some(), n.frame())
                }
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
                        if child_bottom > 0.0 && child_extends_below_parent && rel_bottom > node_frame.h {
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
                    uix_platform::log::debug_fn(format!("[Layout] Phase 2: id={} frame_h {:.0} → {:.0} (child bottom={:.0})",
                        id, node_frame.h, effective_h, max_bottom,));
                    if let Some(_node_mut) = self.get_mut(id) {
                        self.set_frame_dirty(id, Rect::new(old_frame.x, old_frame.y, old_frame.w, effective_h));
                    }
                } else if has_resized_child {
                    uix_platform::log::debug_fn(format!("[Layout] Phase 2: id={} re-layout siblings (child resized, frame_h={:.0})",
                        id, node_frame.h,));
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
        for &id in &self.dirty_traverse() {
            if let Some(node) = self.get(id) {
                if node.children_clip(node.frame()).is_none() {
                    continue;
                }
                let frame = node.frame();
                let children = node.children().to_vec();
                if children.is_empty() {
                    continue;
                }
                uix_platform::log::debug_fn(format!("[Layout] Phase 3: viewport id={} frame=({:.0},{:.0},{:.0},{:.0}) {} children",
                    id, frame.x, frame.y, frame.w, frame.h, children.len(),));
                // 仅触发 content_bounds 副作用，丢弃返回的 child rects
                let _ = node.layout_children(frame, &children, self);
            }
        }
    }

    /// 检查节点是否有 viewport 祖先（如 ScrollView）。
    /// 递归遍历祖先链，不限于直接父节点。
    /// 用于 layout_shrink 中避免收缩 viewport 内部节点，防止与 ScrollView 尺寸设定形成振荡。
    fn has_viewport_ancestor(&self, id: WidgetId) -> bool {
        let mut current = id;
        while let Some(pid) = self.get(current).and_then(|n| n.parent()) {
            if self.get(pid)
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
            struct ShrinkOp { id: WidgetId, needed_h: f32 }
            let mut ops: Vec<ShrinkOp> = Vec::new();

            for &id in rev_order {
                let is_viewport = self.get(id)
                    .map(|n| n.children_clip(n.frame()).is_some())
                    .unwrap_or(false);
                if is_viewport { continue; }
                // 不收缩祖先链中有 viewport（如 ScrollView）的节点，
                // 避免与 ScrollView::layout_children 的尺寸设定形成振荡。
                // 递归检查所有祖先，不限于直接父节点（修复 Container→Input 嵌套场景）。
                if self.has_viewport_ancestor(id) { continue; }
                // layout_viewports（Phase 3）会在收缩后更新 content_bounds。

                let children: Vec<WidgetId> = match self.get(id) {
                    Some(n) if !n.children().is_empty() => n.children().to_vec(),
                    _ => continue,
                };

                // 先按当前 frame 重新布局子节点（兄弟组件靠拢/张开）
                let Some(frame) = self.get(id).map(|n| n.frame()) else { continue; };
                let positions: Vec<(WidgetId, Rect)> = {
                    let Some(node) = self.get(id) else { continue; };
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
                // 使用 1.0 像素绝对最小值而非比例值（如 0.01 * h），
                // 后者在高 DPI 场景下可能过大（2000px * 0.01 = 20px 虚高）。
                let pref_h = self.get(id)
                    .map(|n| n.preferred_size(None).h)
                    .unwrap_or(0.0);
                let min_h = needed_h.max(pref_h).max(1.0);
                let effective_needed = min_h;
                if node_frame.h - effective_needed > 0.5 {
                    uix_platform::log::debug_fn(format!("[Layout] Phase 4: id={} shrink {:.0}px {:.0}→{:.0} (needed={:.0} pref={:.0})",
                        id, node_frame.h - effective_needed, node_frame.h, effective_needed, needed_h, pref_h,));
                    ops.push(ShrinkOp { id, needed_h: effective_needed });
                }
            }
            // Phase B: 执行收缩
            for op in &ops {
                if let Some(old_frame) = self.get(op.id).map(|n| n.frame()) {
                    if let Some(_node_mut) = self.get_mut(op.id) {
                        self.set_frame_dirty(op.id, Rect::new(old_frame.x, old_frame.y, old_frame.w, op.needed_h));
                    }
                    let children: Vec<WidgetId> = self.get(op.id)
                        .map(|n| n.children().to_vec())
                        .unwrap_or_default();
                    let new_frame = Rect::new(old_frame.x, old_frame.y, old_frame.w, op.needed_h);
                    // 收缩后重新布局子节点
                    let new_positions = self.get(op.id)
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
                        let parent_children: Vec<WidgetId> = self.get(pid)
                            .map(|n| n.children().to_vec())
                            .unwrap_or_default();
                        if !parent_children.is_empty() {
                            let parent_positions = self.get(pid)
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
            if !any_changed { break; }
        }
        any_changed
    }

    pub fn update(&mut self, dt: f64) -> bool {
        // ⚠️  使用 traverse() 而非 dirty_traverse()：动画节点在 reset_dirty()
        // 清除脏标记后不会被 dirty_traverse 遍历到，导致 on_update 不再被调用，
        // 动画冻结在第 1 帧。全树遍历确保所有动画节点每帧都能收到 on_update 推进。
        let order = self.traverse();
        let mut any_animating = false;
        for &id in &order {
            // 跳过不可见节点，避免隐藏页面的动画组件拖累全局帧率
            if !self.get(id).map(|n| n.visible()).unwrap_or(false) {
                continue;
            }
            let was_animating = self
                .get(id)
                .map(|n| n.needs_continuous_update())
                .unwrap_or(false);

            // ── 动画帧快照：on_update 前记录旧绘制区域 ──
            // dirty_rect() 返回 widget 的实际绘制区域（含阴影等扩展），
            // 比 frame() 更精确——frame 不变但阴影效果变化时也能追踪。
            // 只对动画 widget 生效，非动画 widget 零开销。
            // 同时对刚启动动画的 widget 也做快照，确保旧帧被正确清除。
            let old_dirty_rect = if was_animating {
                self.get(id).map(|n| n.dirty_rect(n.frame()))
            } else {
                None
            };

            if let Some(node) = self.get_mut(id) {
                node.on_update(dt);
            }

            let (rect, scroll, new_frame, just_started) = self
                .get(id)
                .map(|node| {
                    let is_still = node.needs_continuous_update();
                    if is_still {
                        uix_platform::log::info_fn(format!("[Anim] id={} still animating", node.id()));
                        any_animating = true;
                    }
                    let dirty = if was_animating || is_still {
                        any_animating = any_animating || is_still;
                        node.dirty_rect(node.frame())
                    } else {
                        Rect::zero()
                    };
                    (dirty, node.scroll_delta(node.frame()), node.frame(), is_still && !was_animating)
                })
                .unwrap_or_default();

            // ── 刚启动动画的 widget：也对其旧帧做脏标记 ──
            // 避免首次 on_update 后旧绘制区域未被清除导致视觉残留。
            if just_started && old_dirty_rect.is_none() {
                if let Some(old) = self.get(id).map(|n| n.dirty_rect(n.frame())) {
                    if old != rect && old.w > 0.0 && old.h > 0.0 {
                        self.dirty.region.add_rect(old);
                        if let Some(node) = self.get_mut(id) {
                            node.set_dirty(true);
                        }
                    }
                }
            }

            // ── 动画帧变化追踪：标记旧绘制区域为脏 ──
            // 当 widget 的 dirty_rect 变化（包括 frame 移动、阴影变化等），
            // 旧绘制区域必须被清除，否则产生视觉残留。
            if let Some(old) = old_dirty_rect {
                if old != rect && old.w > 0.0 && old.h > 0.0 {
                    self.dirty.region.add_rect(old);
                    if let Some(node) = self.get_mut(id) {
                        node.set_dirty(true);
                    }
                }
            }

            if rect.w > 0.0 || rect.h > 0.0 {
                self.mark_dirty_rect(id, rect);
            }
            if let Some((dx, dy)) = scroll {
                if dx != 0.0 || dy != 0.0 {
                    self.dirty.scroll_deltas.push((new_frame, dx, dy));
                }
            }
        }

        // ── 可见性同步：组件主动隐藏 → 同步到树级 visible ──
        // on_update 中组件（Modal/Drawer 等）可能修改了 self.visible 为 false
        //（退场动画完成），但树级 BoxedWidget.visible 未更新。
        // 这里单向同步：组件→树，且仅限 comp_visible=false 方向（组件隐藏自己）。
        // 反向（组件想显示）由调用方显式调用 tree.set_visible() 处理，
        // 因为 on_update 不会把隐藏的组件显示出来。
        //
        // 注意：!comp_visible 条件天然阻止了 tree.set_visible(false) 被默认
        // component().visible()=true 反向覆盖——页面容器没有 visible 覆盖，
        // comp.visible 永远为 true，!comp_visible 为 false，不会进入同步。
        let mut sync_list: Vec<(WidgetId, bool)> = Vec::new();
        for &id in &order {
            if let Some(node) = self.get(id) {
                let comp_visible = node.component().visible();
                if node.visible() != comp_visible && !comp_visible {
                    // 组件主动隐藏了自己（如动画完成），同步到 tree 层
                    sync_list.push((id, comp_visible));
                }
            }
        }
        for (id, v) in sync_list {
            self.set_visible(id, v);
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

    /// 按类型查找 widget 并设置焦点（用于 tree.build 后恢复焦点）。
    ///
    /// 遍历当前树查找指定类型的 widget，若找到则设置为聚焦状态。
    /// `focused_widget` 用于键盘事件路由，`Input::set_focused` 控制光标显示。
    pub fn focus_by_type<T: WidgetComponent + 'static>(&mut self) -> Option<WidgetId> {
        let id = self.find_by_type::<T>()?;
        self.focused_widget = Some(id);
        // 对于 Input 类型，同步设置其内部 focused 状态
        if let Some(node) = self.get_mut(id) {
            if let Some(input) = node.component_mut().as_any_mut()
                .downcast_mut::<crate::widgets::Input>()
            {
                input.set_focused(true);
            }
        }
        Some(id)
    }

    /// 检查指定类型的 widget 当前是否处于聚焦状态
    ///
    /// 用于 tree.build 前判断是否需要重建后恢复焦点。
    pub fn is_focused_type<T: WidgetComponent + 'static>(&self) -> bool {
        self.focused_widget
            .and_then(|id| self.get(id))
            .map(|node| node.component().as_any().downcast_ref::<T>().is_some())
            .unwrap_or(false)
    }
}

#[cfg(test)]
#[path = "tree_core_tests.rs"]
mod tests;
