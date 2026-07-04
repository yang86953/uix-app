use super::*;
use crate::managers::EventManager;
use std::collections::HashMap;
use uix_graphics::pipeline::{AnimationRegistry, InvalidationQueueHandle};
use uix_platform::{KeyMod, MouseButton, Point, Rect};

/// 拖拽手势状态，用于从原始鼠标事件组合 DragStart/DragMove/DragEnd。
/// 当 MouseDown 后 MouseMove 超出 5px 阈值时自动识别为拖拽。
#[derive(Clone)]
pub(crate) struct DragGestureState {
    /// MouseDown 已收到且未触发 DragStart
    pub potential: bool,
    /// 拖拽已激活（超出移动阈值）
    pub active: bool,
    /// 拖拽起始位置（屏幕坐标）
    pub start_pos: Point,
    /// 上一次 MouseMove 位置
    pub last_pos: Point,
    /// 触发拖拽的鼠标按钮
    pub button: MouseButton,
    /// 触发拖拽时的修饰键
    pub mods: KeyMod,
    /// 拖拽目标 widget（mouse_down_target）
    pub target: Option<WidgetId>,
}

impl Default for DragGestureState {
    fn default() -> Self {
        Self {
            potential: false,
            active: false,
            start_pos: Point::default(),
            last_pos: Point::default(),
            button: MouseButton::None,
            mods: KeyMod::NONE,
            target: None,
        }
    }
}

impl DragGestureState {
    /// 重置所有状态。
    pub fn reset(&mut self) {
        self.potential = false;
        self.active = false;
        self.button = MouseButton::None;
        self.mods = KeyMod::NONE;
        self.target = None;
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
    /// 滚动 memmove 参数（Composite 失效附带，帧内消费）。
    pub(crate) scroll_region_move: Option<(Rect, f32, f32)>,
    pub(crate) mouse_down_target: Option<WidgetId>,
    /// 树结构版本号，结构变更时递增（add_child / remove / set_root）。
    /// 引擎可用此判断 LayerTree 是否需要重建。
    pub tree_version: u64,
    /// 缓存的先序遍历结果（内部可变性，仅用作性能缓存）。
    /// 当 `cached_traversal_version != tree_version` 时失效重建。
    cached_traversal: std::cell::RefCell<(Vec<WidgetId>, u64)>,

    /// 每个 widget 的独立事件管理器（按需创建）。
    /// 在 `dispatch_to` 中，于 `on_event` 之后自动调用。
    pub(crate) event_managers: HashMap<WidgetId, EventManager>,

    /// 拖拽手势状态：跟踪 MouseDown→Move 序列以产生 DragStart/DragMove/DragEnd。
    /// 拖拽阈值 5px，MouseMove 超出此距离才触发拖拽。
    pub(crate) drag_gesture: DragGestureState,
    /// 渲染失效队列（Phase 2/6：统一 invalidation 入口）。
    pub(crate) invalidation: InvalidationQueueHandle,
    /// 动画注册表（Phase 2：仅 tick 活跃动画节点）。
    pub(crate) animation_registry: AnimationRegistry,
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
            scroll_region_move: None,
            mouse_down_target: None,
            tree_version: 0,
            cached_traversal: std::cell::RefCell::new((Vec::new(), 0)),
            event_managers: HashMap::new(),
            drag_gesture: DragGestureState::default(),
            invalidation: uix_graphics::pipeline::InvalidationQueue::shared(),
            animation_registry: AnimationRegistry::new(),
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
        self.push_layout_invalidation(id);
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

        // 结构变化：Layout 失效向上传播
        self.push_layout_invalidation(parent_id);
        self.propagate_layout_invalidation(parent_id);
        self.try_register_animation(child_id);

        child_id
    }

    pub fn remove(&mut self, id: WidgetId) {
        self.tree_version += 1;

        // 在移除前标记旧 frame 为脏，确保该区域被重绘（清除视觉残留）
        let old_frame = self
            .get(id)
            .map(|n| n.frame())
            .filter(|f| f.w > 0.0 && f.h > 0.0);

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
            if let Some(pid) = parent_id {
                self.invalidate_paint_rect(pid, frame);
            }
        }
        self.animation_registry.unregister(id);

        if let Some(pid) = parent_id {
            self.push_layout_invalidation(pid);
            self.propagate_layout_invalidation(pid);
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
            let children: Vec<WidgetId> = self
                .get(current)
                .map(|n| n.children().to_vec())
                .unwrap_or_default();

            // 先记录 visible 是否变化（get_mut 的借用释放后再标记 dirty）
            let mut changed = false;
            if let Some(n) = self.get_mut(current) {
                if n.visible() != visible {
                    n.set_visible(visible);
                    self.tree_version += 1;
                    changed = true;
                }
            }

            if changed {
                self.invalidate_paint(current);
                self.push_layout_invalidation(current);
            }

            for child in children {
                stack.push(child);
            }
        }

        self.propagate_layout_invalidation(id);
    }

    /// 返回 Layout 失效影响的子树先序遍历顺序。
    ///
    /// 全帧或含 Layout 根时遍历对应子树；无 Layout 失效时返回空（跳过 layout）。
    pub fn layout_traverse(&self) -> Vec<WidgetId> {
        let inv = self
            .invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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

    #[allow(dead_code)]
    /// 兼容旧名；Phase 6 后 layout 使用 `layout_traverse`。
    pub fn dirty_traverse(&self) -> Vec<WidgetId> {
        self.layout_traverse()
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
                if old == new_frame {
                    return;
                }
                old
            }
            None => return,
        };
        if let Some(w) = self.get_mut(id) {
            w.set_frame(new_frame);
        }
        self.invalidate_paint_rect(id, old);
        self.invalidate_paint(id);
        self.push_layout_invalidation(id);
        self.propagate_layout_invalidation(id);
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

        let order = self.layout_traverse();
        if order.is_empty() {
            return;
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
        // Phase 6：layout 完成后用最新 frame 绑定 State → Paint rect
        self.bind_reactive_widget_states();
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
                    uix_platform::log::debug_fn(format!(
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
                    uix_platform::log::debug_fn(format!(
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
                uix_platform::log::debug_fn(format!(
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
                // ⭐ 最小高度取子节点实际内容和 preferred_size 的较大值。
                // 设此下限可防止收缩到子节点内容以下，从而避免与
                // layout_expand（Phase 2）形成振荡循环。
                // 使用 1.0 像素绝对最小值而非比例值（如 0.01 * h），
                // 后者在高 DPI 场景下可能过大（2000px * 0.01 = 20px 虚高）。
                let pref_h = self
                    .get(id)
                    .map(|n| n.preferred_size(None).h)
                    .unwrap_or(0.0);
                let min_h = needed_h.max(pref_h).max(1.0);
                let effective_needed = min_h;
                if node_frame.h - effective_needed > 0.5 {
                    uix_platform::log::debug_fn(format!("[Layout] Phase 4: id={} shrink {:.0}px {:.0}→{:.0} (needed={:.0} pref={:.0})",
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
        // Phase 2：仅 tick AnimationRegistry 中的节点，避免全树 O(n) 扫描。
        let order = self.animation_registry.active_ids();
        let mut any_animating = false;
        for id in order {
            if !self.get(id).map(|n| n.visible()).unwrap_or(false) {
                self.animation_registry.unregister(id);
                continue;
            }
            let was_animating = true;

            let old_dirty_rect = self.get(id).map(|n| n.dirty_rect(n.frame()));

            if let Some(node) = self.get_mut(id) {
                node.on_update(dt);
            }

            let (rect, just_started, is_still) = self
                .get(id)
                .map(|node| {
                    let is_still = node.needs_continuous_update();
                    let dirty = if was_animating || is_still {
                        node.dirty_rect(node.frame())
                    } else {
                        Rect::zero()
                    };
                    (dirty, is_still && !was_animating, is_still)
                })
                .unwrap_or((Rect::zero(), false, false));

            if is_still {
                any_animating = true;
            } else {
                self.animation_registry.unregister(id);
            }

            if just_started && old_dirty_rect.is_none() {
                if let Some(old) = self.get(id).map(|n| n.dirty_rect(n.frame())) {
                    if old != rect && old.w > 0.0 && old.h > 0.0 {
                        self.mark_dirty_rect(id, old);
                    }
                }
            }

            if let Some(old) = old_dirty_rect {
                if old != rect && old.w > 0.0 && old.h > 0.0 {
                    self.mark_dirty_rect(id, old);
                }
            }

            if rect.w > 0.0 || rect.h > 0.0 {
                self.mark_dirty_rect(id, rect);
            }

            // 滚动时使用整视口 Paint 失效，避免 scroll_region memmove 与 strip
            // 剪枝不同步导致内容间歇性消失。
            if let Some((dx, dy)) = self.get(id).and_then(|n| n.scroll_delta_for_dirty()) {
                if dx.abs() > 0.5 || dy.abs() > 0.5 {
                    if let Some(node) = self.get(id) {
                        let frame = node.frame();
                        if frame.w > 0.0 && frame.h > 0.0 {
                            self.invalidate_paint_rect(id, frame);
                        }
                    }
                }
            }
        }

        // 可见性同步：仅检查动画注册表节点（Modal/Drawer 退场等）。
        let mut sync_list: Vec<(WidgetId, bool)> = Vec::new();
        for id in self.animation_registry.active_ids() {
            if let Some(node) = self.get(id) {
                let comp_visible = node.component().visible();
                if node.visible() != comp_visible && !comp_visible {
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
        let old_children: Vec<WidgetId> = self
            .get(parent_id)
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
            // 优先使用 WidgetNode 的 tab_index，否则使用组件默认值
            let ti = if node.tab_idx != 0 {
                node.tab_idx
            } else {
                n.component().tab_index()
            };
            n.set_tab_index(ti);
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
            if let Some(input) = node
                .component_mut()
                .as_any_mut()
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

    // ── 事件管理器 ───────────────────────────────────────────

    /// 获取指定 widget 的事件管理器（不存在则创建）。
    pub fn event_manager_for(&mut self, id: WidgetId) -> &mut EventManager {
        self.event_managers.entry(id).or_default()
    }

    /// 移除指定 widget 的事件管理器。
    pub fn remove_event_manager(&mut self, id: WidgetId) {
        self.event_managers.remove(&id);
    }

    /// 清空所有事件管理器。
    pub fn clear_event_managers(&mut self) {
        self.event_managers.clear();
    }

    // ── Tab 键焦点导航 ─────────────────────────────────────────

    /// 收集所有可聚焦的 widget，按 tab_index 升序排序。
    /// 不可见、无 EVENT 能力、tab_index <= 0 的 widget 被排除。
    pub fn collect_focusable(&self) -> Vec<WidgetId> {
        let mut result: Vec<(i32, WidgetId)> = Vec::new();
        for id in self.traverse() {
            if let Some(node) = self.get(id) {
                if node.is_focusable() {
                    result.push((node.tab_index(), id));
                }
            }
        }
        // 按 tab_index 升序排序（小数字先聚焦）
        result.sort_by_key(|&(idx, _)| idx);
        result.into_iter().map(|(_, id)| id).collect()
    }

    /// 查找当前焦点 widget 在可聚焦列表中的位置，返回下一个可聚焦的 ID。
    /// `forward = true` 表示 Tab（向后），false 表示 Shift+Tab（向前）。
    pub fn focus_next(&self, forward: bool) -> Option<WidgetId> {
        let focusable = self.collect_focusable();
        if focusable.is_empty() {
            return None;
        }
        let current = self.focused_widget;
        if let Some(cur_id) = current {
            let pos = focusable.iter().position(|&id| id == cur_id);
            match pos {
                Some(p) => {
                    if forward {
                        Some(focusable[(p + 1) % focusable.len()])
                    } else {
                        Some(focusable[(p + focusable.len() - 1) % focusable.len()])
                    }
                }
                None => Some(focusable[0]), // 当前焦点不在列表中，回到第一个
            }
        } else {
            Some(focusable[0]) // 无焦点，默认聚焦第一个
        }
    }
}

#[cfg(test)]
#[path = "tree_core_tests.rs"]
mod tests;
