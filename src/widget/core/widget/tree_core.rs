use super::*;
use crate::widget::managers::EventManager;
use std::collections::HashMap;
use crate::render::pipeline::{AnimationRegistry, InvalidationQueueHandle};
use crate::platform::{KeyMod, MouseButton, Point, Rect};

#[path = "tree_layout.rs"]
mod tree_layout;

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
            invalidation: crate::render::pipeline::InvalidationQueue::shared(),
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
                .downcast_mut::<crate::widget::widgets::Input>()
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
