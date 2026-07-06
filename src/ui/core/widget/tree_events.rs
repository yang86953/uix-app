use super::tree_core::WidgetTree;
use super::*;
use crate::ui::event::{ClickEvent, SemanticEvent};
use crate::ui::{OverlayEntry, OverlayKind};

impl WidgetTree {
    /// 2D 命中测试：根据屏幕坐标找到最深的 widget。
    pub fn hit_test(&self, pos: Point) -> Option<WidgetId> {
        self.root_id
            .and_then(|root| self.hit_test_internal(root, pos))
    }

    /// 3D 命中测试：根据 3D 射线找到最深的 widget。
    ///
    /// `spatial` 为当前空间上下文（用于逆变换）。
    /// 使用 Widget::hit_test_3d 方法，支持 3D 变换后的 widget。
    pub fn hit_test_3d(
        &self,
        ray: &crate::draw::spatial::Ray3D,
        spatial: &crate::draw::spatial::SpatialContext,
    ) -> Option<WidgetId> {
        self.root_id
            .and_then(|root| self.hit_test_3d_internal(root, ray, spatial))
    }

    /// 3D 命中测试内部递归。
    fn hit_test_3d_internal(
        &self,
        id: WidgetId,
        ray: &crate::draw::spatial::Ray3D,
        spatial: &crate::draw::spatial::SpatialContext,
    ) -> Option<WidgetId> {
        let node = self.get(id)?;
        if !node.visible() {
            return None;
        }
        let mut sorted: Vec<WidgetId> = node.children().to_vec();
        sorted.sort_by(|&a, &b| {
            let za = self.get(a).map_or(0, |c| c.z_index());
            let zb = self.get(b).map_or(0, |c| c.z_index());
            zb.cmp(&za)
        });
        for &child_id in &sorted {
            if let Some(hit) = self.hit_test_3d_internal(child_id, ray, spatial) {
                return Some(hit);
            }
        }
        let frame = node.frame();
        if node.hit_test_3d(ray, spatial, frame) {
            Some(id)
        } else {
            None
        }
    }

    /// 获取 viewport 容器的 scroll 偏移（用于 hit_test 补偿）。
    fn get_scroll_offset(tree: &WidgetTree, id: WidgetId) -> Option<(f32, f32)> {
        tree.get(id).and_then(|n| n.viewport_scroll_offset())
    }

    fn hit_test_internal(&self, id: WidgetId, pos: Point) -> Option<WidgetId> {
        let node = self.get(id)?;
        if !node.visible() {
            return None;
        }

        // 如果当前节点是 ScrollView，对其子节点做 scroll offset 补偿。
        // ScrollView 的子节点按自然坐标布局，但渲染时通过 canvas translate(-sx, -sy) 偏移。
        // hit_test 必须补偿这个偏移，否则滚动后点击会定位到错误位置。
        let scroll_off = Self::get_scroll_offset(self, id);
        let child_pos = match scroll_off {
            Some((sx, sy)) => Point::new(pos.x + sx, pos.y + sy),
            None => pos,
        };

        let mut sorted: Vec<WidgetId> = node.children().to_vec();
        sorted.sort_by(|&a, &b| {
            let za = self.get(a).map_or(0, |c| c.z_index());
            let zb = self.get(b).map_or(0, |c| c.z_index());
            zb.cmp(&za)
        });
        for &child_id in &sorted {
            if let Some(hit) = self.hit_test_internal(child_id, child_pos) {
                return Some(hit);
            }
        }
        // 使用 widget 的 hit_test_frame 代替原始 frame，支持 overlay 模式
        let actual_frame = node.frame();
        let hit_frame = node.hit_test_frame(actual_frame);
        if hit_frame.contains(pos) {
            Some(id)
        } else {
            None
        }
    }

    fn overlay_target_at(&self, pos: Point) -> Option<WidgetId> {
        self.overlay_stack
            .hit_test(pos.x, pos.y)
            .map(|entry| entry.owner())
    }

    fn intercept_top_overlay_outside_pointer_down(&mut self, pos: Point) -> Option<EventResult> {
        let top = self.overlay_stack.top().cloned()?;
        let inside_top = top.bounds_rect().is_some_and(|bounds| bounds.contains(pos));
        if inside_top {
            return None;
        }

        if top.is_modal() || top.traps_focus() || top.dismisses_on_outside() {
            if top.dismisses_on_outside() {
                self.overlay_stack.remove(top.id());
                self.invalidate_paint(top.owner());
            }
            self.set_focus(None);
            return Some(EventResult::Handled);
        }

        None
    }

    fn open_context_menu_overlay(&mut self, owner: WidgetId, pos: Point) {
        self.overlay_stack
            .retain_entries(|entry| entry.kind() != OverlayKind::ContextMenu);
        self.overlay_stack.push_entry(
            OverlayEntry::new(owner, OverlayKind::ContextMenu)
                .bounds(Rect::new(pos.x, pos.y, 160.0, 160.0))
                .z_index(1200)
                .dismiss_on_outside(true),
        );
        self.invalidate_paint(owner);
    }

    pub fn dispatch_event(&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, button, mods } => {
                if let Some(result) = self.intercept_top_overlay_outside_pointer_down(*pos) {
                    return result;
                }
                let target = self.overlay_target_at(*pos).or_else(|| self.hit_test(*pos));
                self.pointer_down_target = target;
                // 记录拖拽起始状态
                self.drag_gesture.potential = true;
                self.drag_gesture.start_pos = *pos;
                self.drag_gesture.last_pos = *pos;
                self.drag_gesture.button = *button;
                self.drag_gesture.mods = *mods;
                self.drag_gesture.target = target;
                if let Some(t) = target {
                    self.invalidate_paint(t);
                    // 捕获阶段：root → target，用于 Modal 等拦截
                    if self.capture_to(t, event) == EventResult::Handled {
                        return EventResult::Handled;
                    }
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled {
                        self.invalidate_nav_siblings(t);
                        self.set_focus(Some(t));
                    } else {
                        // 点击不处理事件的 widget → 取消焦点
                        self.set_focus(None);
                    }
                    self.rebuild_widget_overlays();
                    result
                } else {
                    self.set_focus(None);
                    self.rebuild_widget_overlays();
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerUp { pos, button, mods } => {
                let hold = self.pointer_down_target;
                // 如果拖拽处于活跃状态，发射 DragEnd 到拖拽目标
                if self.drag_gesture.active {
                    if let Some(target) = self.drag_gesture.target {
                        self.invalidate_paint(target);
                        let drag_end = SystemEvent::DragEnd {
                            pos: *pos,
                            button: *button,
                            mods: *mods,
                        };
                        let _ = self.dispatch_to(target, &drag_end);
                    }
                }
                self.drag_gesture.reset();
                self.pointer_down_target = None;
                let hit = self.overlay_target_at(*pos).or_else(|| self.hit_test(*pos));
                let mut result = EventResult::NotHandled;
                if let Some(t) = hit {
                    self.invalidate_paint(t);
                    // 捕获阶段：root → target
                    if self.capture_to(t, event) == EventResult::Handled {
                        return EventResult::Handled;
                    }
                    result = self.dispatch_to(t, event);
                    if result == EventResult::Handled && hold == Some(t) {
                        let click = ClickEvent {
                            button: *button,
                            pos: *pos,
                            modifiers: *mods,
                        };
                        let _ = self.dispatch_semantic(SemanticEvent::click(t, click));
                        if *button == MouseButton::Right {
                            let mut context_menu = SemanticEvent::context_menu(t, click);
                            let _ = self.dispatch_semantic_event(&mut context_menu);
                            if !context_menu.default_prevented() {
                                self.open_context_menu_overlay(t, click.pos);
                            }
                        }
                    }
                }
                if let Some(t) = hold {
                    if Some(t) != hit {
                        self.invalidate_paint(t);
                        let _ = self.dispatch_to(t, event);
                    }
                }
                self.rebuild_widget_overlays();
                result
            }
            SystemEvent::PointerMove { pos, mods } => {
                // 拖拽手势检测：potential → active 转换
                if self.drag_gesture.potential && !self.drag_gesture.active {
                    let dx = pos.x - self.drag_gesture.start_pos.x;
                    let dy = pos.y - self.drag_gesture.start_pos.y;
                    // 5px 阈值：超出才视为拖拽开始
                    if dx.abs() > 5.0 || dy.abs() > 5.0 {
                        self.drag_gesture.active = true;
                        self.drag_gesture.potential = false;
                        // 发射 DragStart 到拖拽目标
                        if let Some(target) = self.drag_gesture.target {
                            self.invalidate_paint(target);
                            let drag_start = SystemEvent::DragStart {
                                pos: *pos,
                                button: self.drag_gesture.button,
                                mods: *mods,
                            };
                            let _ = self.dispatch_to(target, &drag_start);
                        }
                    }
                }
                // 拖拽进行中：发射 DragMove
                if self.drag_gesture.active {
                    if let Some(target) = self.drag_gesture.target {
                        self.invalidate_paint(target);
                        let delta = Point::new(
                            pos.x - self.drag_gesture.last_pos.x,
                            pos.y - self.drag_gesture.last_pos.y,
                        );
                        let drag_move = SystemEvent::DragMove {
                            pos: *pos,
                            delta,
                            mods: *mods,
                        };
                        let _ = self.dispatch_to(target, &drag_move);
                    }
                }
                self.drag_gesture.last_pos = *pos;

                let new_hover = self.overlay_target_at(*pos).or_else(|| self.hit_test(*pos));
                if new_hover != self.hovered_widget {
                    if let Some(old) = self.hovered_widget {
                        let _ = self.dispatch_to(old, &SystemEvent::PointerLeave);
                        self.invalidate_paint(old);
                    }
                    if let Some(new) = new_hover {
                        let _ = self.dispatch_to(new, &SystemEvent::PointerEnter);
                        self.invalidate_paint(new);
                    }
                    self.hovered_widget = new_hover;
                }
                // 拖拽期间同时分发 PointerMove 给 pointer_down_target
                // （支持文字选中、滑动条拖拽等跨边界操作）
                // 注意：在拖拽活跃时，PointerMove 原始事件仍然分发，
                // 以便使用 pointer_down_target 的 scrollbar/slider 等仍能工作
                if let Some(drag_target) = self.pointer_down_target {
                    self.invalidate_paint(drag_target);
                    let _ = self.dispatch_to(drag_target, event);
                }
                let result = if let Some(t) = new_hover {
                    self.dispatch_to(t, event)
                } else {
                    EventResult::NotHandled
                };
                self.rebuild_widget_overlays();
                result
            }
            SystemEvent::Wheel { pos, .. } => {
                let target = self
                    .overlay_target_at(*pos)
                    .or_else(|| self.hit_test(*pos))
                    .or(self.hovered_widget)
                    .or(self.root_id);
                if let Some(t) = target {
                    // 捕获阶段：ScrollView 等祖先先处理；Handled 时由 capture 侧登记动画与视口重绘，
                    // 避免仅 mark_dirty 子节点导致 strip 局部清除后内容消失。
                    if self.capture_to(t, event) == EventResult::Handled {
                        return EventResult::Handled;
                    }
                    self.invalidate_paint(t);
                    self.dispatch_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyDown { key, mods } => {
                // Tab 键焦点导航（在捕获和冒泡之前处理）
                if *key == KeyCode::Tab {
                    let forward = !mods.contains(KeyMod::SHIFT);
                    if let Some(next) = self.focus_next(forward) {
                        self.set_focus(Some(next));
                        return EventResult::Handled;
                    }
                    return EventResult::NotHandled;
                }

                if let Some(t) = self.focused_widget {
                    self.invalidate_paint(t);
                    // 捕获阶段：root → target，用于全局快捷键
                    if self.capture_to(t, event) == EventResult::Handled {
                        return EventResult::Handled;
                    }
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled
                        && matches!(*key, KeyCode::Enter | KeyCode::Space)
                    {
                        let click = ClickEvent {
                            button: MouseButton::Left,
                            pos: self
                                .get(t)
                                .map(|node| {
                                    let frame = node.frame();
                                    Point::new(frame.x + frame.w * 0.5, frame.y + frame.h * 0.5)
                                })
                                .unwrap_or_default(),
                            modifiers: *mods,
                        };
                        let _ = self.dispatch_semantic(SemanticEvent::click(t, click));
                    }
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyUp { .. } => {
                if let Some(t) = self.focused_widget {
                    self.invalidate_paint(t);
                    // 捕获阶段：root → target
                    if self.capture_to(t, event) == EventResult::Handled {
                        return EventResult::Handled;
                    }
                    self.dispatch_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::TextInput { text } => {
                if let Some(t) = self.focused_widget {
                    self.invalidate_paint(t);
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled {
                        let _ = self.dispatch_semantic(SemanticEvent::text_input(t, text.clone()));
                    }
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::Copy | SystemEvent::Cut | SystemEvent::Paste { .. } => {
                if let Some(t) = self.focused_widget {
                    self.invalidate_paint(t);
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled {
                        let semantic = match event {
                            SystemEvent::Copy => SemanticEvent::copy(t),
                            SystemEvent::Cut => SemanticEvent::cut(t),
                            SystemEvent::Paste { text } => SemanticEvent::paste(t, text.clone()),
                            _ => unreachable!(),
                        };
                        let _ = self.dispatch_semantic(semantic);
                    }
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn | SystemEvent::FocusOut => {
                if let Some(t) = self.focused_widget {
                    self.dispatch_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerEnter | SystemEvent::PointerLeave => EventResult::NotHandled,
            SystemEvent::ThemeChanged { .. } => {
                self.notify_theme_changed();
                if let Some(root) = self.root_id {
                    self.dispatch_to(root, event)
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::LocaleChanged { .. } => {
                if let Some(root) = self.root_id {
                    self.push_layout_invalidation(root);
                    self.invalidate_paint(root);
                    self.dispatch_to(root, event)
                } else {
                    EventResult::NotHandled
                }
            }
            // 窗口状态变化事件 → 统一分发给 root，让应用层处理
            SystemEvent::WindowMaximize
            | SystemEvent::WindowMinimize
            | SystemEvent::WindowRestore
            | SystemEvent::WindowFocus
            | SystemEvent::WindowBlur => {
                if let Some(root) = self.root_id {
                    self.dispatch_to(root, event)
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::Timer { .. } => {
                let target = self.hovered_widget.or(self.focused_widget).or(self.root_id);
                if let Some(target) = target {
                    let result = self.dispatch_to(target, event);
                    self.rebuild_widget_overlays();
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FileDrop { files, position } => {
                // 文件拖放优先分发给命中节点，否则交给 root。
                if let Some(target) = self.hit_test(*position).or(self.root_id) {
                    let result = self.dispatch_to(target, event);
                    let _ = self.dispatch_semantic(SemanticEvent::file_drop(
                        target,
                        files.clone(),
                        *position,
                    ));
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            // 拖拽组合事件：由 dispatch_event 内部合成并直接 dispatch_to，
            // 不会从外部传入 dispatch_event。
            SystemEvent::DragStart { .. }
            | SystemEvent::DragMove { .. }
            | SystemEvent::DragEnd { .. } => EventResult::NotHandled,
            SystemEvent::Resize { width, height } => {
                if let Some(root) = self.root_id {
                    if *width > 0.0 && *height > 0.0 {
                        if let Some(root_mut) = self.get_mut(root) {
                            root_mut.set_frame(Rect::new(0.0, 0.0, *width, *height));
                            self.invalidate_paint(root);
                        }
                        // 递增 tree_version 使 LayerTree 重建
                        // LayerTree 缓存了 ClipRect（如 ScrollView 的裁剪矩形），
                        // 不重建则子节点位置更新了但裁剪区还是旧尺寸 → 内容被裁剪。
                        self.tree_version += 1;
                    }
                    self.dispatch_to(root, event)
                } else {
                    EventResult::NotHandled
                }
            }
        }
    }

    /// 计算从目标到根路径上所有 ScrollView 的累计滚动偏移。
    fn cumulative_scroll_offset(&self, target: WidgetId) -> Option<(f32, f32)> {
        let mut sx = 0.0f32;
        let mut sy = 0.0f32;
        let mut found = false;
        let mut current = Some(target);
        while let Some(id) = current {
            if let Some((ox, oy)) = Self::get_scroll_offset(self, id) {
                sx += ox;
                sy += oy;
                found = true;
            }
            current = self.get(id).and_then(|n| n.parent());
        }
        if found {
            Some((sx, sy))
        } else {
            None
        }
    }

    /// 在 PointerDown/PointerUp/PointerMove 事件位置上增加偏移量。
    fn add_offset_to_event(event: SystemEvent, sx: f32, sy: f32) -> SystemEvent {
        match event {
            SystemEvent::PointerDown { pos, button, mods } => SystemEvent::PointerDown {
                pos: Point::new(pos.x + sx, pos.y + sy),
                button,
                mods,
            },
            SystemEvent::PointerUp { pos, button, mods } => SystemEvent::PointerUp {
                pos: Point::new(pos.x + sx, pos.y + sy),
                button,
                mods,
            },
            SystemEvent::PointerMove { pos, mods } => SystemEvent::PointerMove {
                pos: Point::new(pos.x + sx, pos.y + sy),
                mods,
            },
            other => other,
        }
    }

    /// 捕获阶段：从 root 到 target 的路径上依次分发事件（不含 target 自身）。
    /// 任意节点返回 `Handled` 则终止捕获并阻止后续冒泡阶段。
    /// 用于 Modal 外部点击拦截、ScrollView 滚动拦截、全局快捷键等场景。
    fn capture_to(&mut self, target: WidgetId, event: &SystemEvent) -> EventResult {
        // 收集从 root 到 target 的祖先路径（不含 target）
        let mut path = Vec::new();
        let mut current = self.get(target).and_then(|n| n.parent());
        while let Some(id) = current {
            path.push(id);
            current = self.get(id).and_then(|n| n.parent());
        }
        path.reverse(); // 现在是从 root → ... → target.parent

        for &id in &path {
            let handled = {
                let node = match self.get_mut(id) {
                    Some(n) => n,
                    None => continue,
                };
                node.on_event(event) == EventResult::Handled
            };
            if handled {
                self.on_widget_handled_in_capture(id);
                return EventResult::Handled;
            }
        }
        EventResult::NotHandled
    }

    /// capture 阶段拦截事件后：标记拦截节点重绘。
    fn on_widget_handled_in_capture(&mut self, id: WidgetId) {
        self.invalidate_paint(id);
    }

    fn dispatch_to(&mut self, target: WidgetId, event: &SystemEvent) -> EventResult {
        // 分发前标记目标为脏
        self.invalidate_paint(target);

        let mut current = Some(target);
        // ScrollView 的子节点框架是自然坐标（未含滚动偏移），
        // 必须先计算目标路径上所有 ScrollView 的累计偏移量，
        // 翻译事件后加上该偏移量，使事件坐标与视觉位置一致。
        let scroll_off = self.cumulative_scroll_offset(target);
        while let Some(id) = current {
            // 先读取 frame（共享借用），传入 translate_pointer_event
            // 再获取可变引用调用 on_event，确保 &mut self 借用不重叠
            let frame = match self.get(id) {
                Some(n) => n.frame(),
                None => return EventResult::NotHandled,
            };
            let translated = Self::translate_pointer_event(event, frame);
            let compensated = match scroll_off {
                Some((sx, sy)) => Self::add_offset_to_event(translated, sx, sy),
                None => translated,
            };

            // 处理 widget 自身的 on_event
            let (result, parent_id) = {
                let node = match self.get_mut(id) {
                    Some(n) => n,
                    None => return EventResult::NotHandled,
                };
                let r = node.on_event(&compensated);
                (r, node.parent())
            };
            if result == EventResult::Handled {
                let semantic = self
                    .get(id)
                    .and_then(|node| node.semantic_event(id, &compensated));
                if let Some(event) = semantic {
                    let _ = self.dispatch_semantic(event);
                }
                return EventResult::Handled;
            }

            // Bubbled 或 NotHandled → 继续向父节点传播
            current = parent_id;
        }
        EventResult::NotHandled
    }

    fn semantic_path_to_root(&self, target: WidgetId) -> Vec<WidgetId> {
        let mut path = Vec::new();
        let mut current = Some(target);
        while let Some(id) = current {
            path.push(id);
            current = self.get(id).and_then(|node| node.parent());
        }
        path
    }

    pub fn dispatch_semantic_event(&mut self, event: &mut SemanticEvent) -> EventResult {
        let path = self.semantic_path_to_root(event.target);
        self.handler_table.dispatch_path(&path, event)
    }

    pub fn dispatch_semantic(&mut self, mut event: SemanticEvent) -> EventResult {
        self.dispatch_semantic_event(&mut event)
    }

    fn translate_pointer_event(event: &SystemEvent, frame: Rect) -> SystemEvent {
        match *event {
            SystemEvent::PointerDown { pos, button, mods } => SystemEvent::PointerDown {
                pos: Point::new(pos.x - frame.x, pos.y - frame.y),
                button,
                mods,
            },
            SystemEvent::PointerUp { pos, button, mods } => SystemEvent::PointerUp {
                pos: Point::new(pos.x - frame.x, pos.y - frame.y),
                button,
                mods,
            },
            SystemEvent::PointerMove { pos, mods } => SystemEvent::PointerMove {
                pos: Point::new(pos.x - frame.x, pos.y - frame.y),
                mods,
            },
            ref other => other.clone(),
        }
    }

    fn set_focus(&mut self, new_focus: Option<WidgetId>) {
        if new_focus == self.focused_widget {
            return;
        }
        if let Some(old) = self.focused_widget {
            self.invalidate_paint(old);
            let _ = self.dispatch_to(old, &SystemEvent::FocusOut);
        }
        self.focused_widget = new_focus;
        if let Some(new) = new_focus {
            self.invalidate_paint(new);
            let _ = self.dispatch_to(new, &SystemEvent::FocusIn);
        }
        self.reconcile_lifecycle_after_layout();
    }

    /// NavItem 共享 active 索引时，刷新整组导航项（取消/选中态联动）。
    fn invalidate_nav_siblings(&mut self, clicked: WidgetId) {
        use crate::ui::widgets::nav::NavItem;
        let is_nav = self
            .get(clicked)
            .is_some_and(|n| n.component().as_any().type_id() == std::any::TypeId::of::<NavItem>());
        if !is_nav {
            return;
        }
        if let Some(parent) = self.get(clicked).and_then(|n| n.parent()) {
            self.invalidate_paint_subtree(parent);
        }
    }
}
