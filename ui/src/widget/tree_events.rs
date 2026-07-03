use super::tree_core::WidgetTree;
use super::*;
use crate::widgets::scroll_view::ScrollView;

impl WidgetTree {
    /// 2D 命中测试：根据屏幕坐标找到最深的 widget。
    pub fn hit_test(&self, pos: Point) -> Option<WidgetId> {
        self.root_id.and_then(|root| self.hit_test_internal(root, pos))
    }

    /// 3D 命中测试：根据 3D 射线找到最深的 widget。
    ///
    /// `spatial` 为当前空间上下文（用于逆变换）。
    /// 使用 Widget::hit_test_3d 方法，支持 3D 变换后的 widget。
    pub fn hit_test_3d(&self, ray: &uix_graphics::spatial::Ray3D, spatial: &uix_graphics::spatial::SpatialContext) -> Option<WidgetId> {
        self.root_id.and_then(|root| self.hit_test_3d_internal(root, ray, spatial))
    }

    /// 3D 命中测试内部递归。
    fn hit_test_3d_internal(&self, id: WidgetId, ray: &uix_graphics::spatial::Ray3D, spatial: &uix_graphics::spatial::SpatialContext) -> Option<WidgetId> {
        let node = self.get(id)?;
        if !node.visible() { return None; }
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

    /// 获取 ScrollView 的滚动偏移量（用于 hit_test 补偿）。
    fn get_scroll_offset(tree: &WidgetTree, id: WidgetId) -> Option<(f32, f32)> {
        tree.get(id).and_then(|node| {
            let comp = node.component();
            let sv = comp.as_any().downcast_ref::<ScrollView>()?;
            if sv.scroll_x().abs() > 0.5 || sv.scroll_y().abs() > 0.5 {
                Some((sv.scroll_x(), sv.scroll_y()))
            } else {
                None
            }
        })
    }

    fn hit_test_internal(&self, id: WidgetId, pos: Point) -> Option<WidgetId> {
        let node = self.get(id)?;
        if !node.visible() { return None; }

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
            if let Some(hit) = self.hit_test_internal(child_id, child_pos) { return Some(hit); }
        }
        // 使用 widget 的 hit_test_frame 代替原始 frame，支持 overlay 模式
        let actual_frame = node.frame();
        let hit_frame = node.hit_test_frame(actual_frame);
        if hit_frame.contains(pos) { Some(id) } else { None }
    }

    pub fn dispatch_event(&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                let target = self.hit_test(*pos);
                self.mouse_down_target = target;
                if let Some(t) = target {
                    self.mark_dirty(t);
                    // 捕获阶段：root → target，用于 Modal 等拦截
                    if self.capture_to(t, event) == EventResult::Handled {
                        return EventResult::Handled;
                    }
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled {
                        self.set_focus(Some(t));
                    } else {
                        // 点击不处理事件的 widget → 取消焦点
                        self.set_focus(None);
                    }
                    result
                } else { self.set_focus(None); EventResult::NotHandled }
            }
            WidgetEvent::MouseUp { pos, .. } => {
                let hold = self.mouse_down_target;
                self.mouse_down_target = None;
                let hit = self.hit_test(*pos);
                let mut result = EventResult::NotHandled;
                if let Some(t) = hit {
                    self.mark_dirty(t);
                    // 捕获阶段：root → target
                    if self.capture_to(t, event) == EventResult::Handled {
                        return EventResult::Handled;
                    }
                    result = self.dispatch_to(t, event);
                }
                if let Some(t) = hold {
                    if Some(t) != hit { self.mark_dirty(t); let _ = self.dispatch_to(t, event); }
                }
                result
            }
            WidgetEvent::MouseMove { pos, .. } => {
                let new_hover = self.hit_test(*pos);
                if new_hover != self.hovered_widget {
                    if let Some(old) = self.hovered_widget {
                        let _ = self.dispatch_to(old, &WidgetEvent::HoverLeave);
                        self.mark_dirty(old);
                    }
                    if let Some(new) = new_hover {
                        let _ = self.dispatch_to(new, &WidgetEvent::HoverEnter);
                        self.mark_dirty(new);
                    }
                    self.hovered_widget = new_hover;
                }
                // 拖拽期间同时分发 MouseMove 给 mouse_down_target
                // （支持文字选中、滑动条拖拽等跨边界操作）
                if let Some(drag_target) = self.mouse_down_target {
                    self.mark_dirty(drag_target);
                    let _ = self.dispatch_to(drag_target, event);
                }
                if let Some(t) = new_hover { self.dispatch_to(t, event) }
                else { EventResult::NotHandled }
            }
            WidgetEvent::MouseWheel { pos, .. } => {
                let target = self.hit_test(*pos).or(self.hovered_widget).or(self.root_id);
                if let Some(t) = target {
                    self.mark_dirty(t);
                    // 捕获阶段：root → target，ScrollView 在此拦截滚动
                    if self.capture_to(t, event) == EventResult::Handled {
                        return EventResult::Handled;
                    }
                    self.dispatch_to(t, event)
                }
                else { EventResult::NotHandled }
            }
            WidgetEvent::KeyDown { .. } | WidgetEvent::KeyUp { .. } => {
                if let Some(t) = self.focused_widget {
                    self.mark_dirty(t);
                    // 捕获阶段：root → target，用于全局快捷键
                    if self.capture_to(t, event) == EventResult::Handled {
                        return EventResult::Handled;
                    }
                    self.dispatch_to(t, event)
                }
                else { EventResult::NotHandled }
            }
            WidgetEvent::KeyPress { .. } => {
                if let Some(t) = self.focused_widget { self.mark_dirty(t); self.dispatch_to(t, event) }
                else { EventResult::NotHandled }
            }
            WidgetEvent::FocusIn | WidgetEvent::FocusOut => {
                if let Some(t) = self.focused_widget { self.dispatch_to(t, event) }
                else { EventResult::NotHandled }
            }
            WidgetEvent::HoverEnter | WidgetEvent::HoverLeave => EventResult::NotHandled,
            // 窗口状态变化事件 → 统一分发给 root，让应用层处理
            WidgetEvent::WindowMaximize
            | WidgetEvent::WindowMinimize
            | WidgetEvent::WindowRestore
            | WidgetEvent::WindowFocus
            | WidgetEvent::WindowBlur => {
                if let Some(root) = self.root_id { self.dispatch_to(root, event) }
                else { EventResult::NotHandled }
            }
            WidgetEvent::Timer { .. } => {
                // 定时器事件分发给 root
                if let Some(root) = self.root_id { self.dispatch_to(root, event) }
                else { EventResult::NotHandled }
            }
            WidgetEvent::FileDrop { .. } => {
                // 文件拖放分发给 root
                if let Some(root) = self.root_id { self.dispatch_to(root, event) }
                else { EventResult::NotHandled }
            }
            WidgetEvent::Resize { width, height } => {
                if let Some(root) = self.root_id {
                    if *width > 0.0 && *height > 0.0 {
                        if let Some(root_mut) = self.get_mut(root) {
                            root_mut.set_frame(Rect::new(0.0, 0.0, *width, *height));
                            self.mark_dirty(root);
                        }
                        // 递增 tree_version 使 LayerTree 重建
                        // LayerTree 缓存了 ClipRect（如 ScrollView 的裁剪矩形），
                        // 不重建则子节点位置更新了但裁剪区还是旧尺寸 → 内容被裁剪。
                        self.tree_version += 1;
                    }
                    self.dispatch_to(root, event)
                } else { EventResult::NotHandled }
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
        if found { Some((sx, sy)) } else { None }
    }

    /// 在 MouseDown/MouseUp/MouseMove 事件位置上增加偏移量。
    fn add_offset_to_event(event: WidgetEvent, sx: f32, sy: f32) -> WidgetEvent {
        match event {
            WidgetEvent::MouseDown { pos, button, mods } =>
                WidgetEvent::MouseDown { pos: Point::new(pos.x + sx, pos.y + sy), button, mods },
            WidgetEvent::MouseUp { pos, button, mods } =>
                WidgetEvent::MouseUp { pos: Point::new(pos.x + sx, pos.y + sy), button, mods },
            WidgetEvent::MouseMove { pos, mods } =>
                WidgetEvent::MouseMove { pos: Point::new(pos.x + sx, pos.y + sy), mods },
            other => other,
        }
    }

    /// 捕获阶段：从 root 到 target 的路径上依次分发事件（不含 target 自身）。
    /// 任意节点返回 `Handled` 则终止捕获并阻止后续冒泡阶段。
    /// 用于 Modal 外部点击拦截、ScrollView 滚动拦截、全局快捷键等场景。
    fn capture_to(&mut self, target: WidgetId, event: &WidgetEvent) -> EventResult {
        // 收集从 root 到 target 的祖先路径（不含 target）
        let mut path = Vec::new();
        let mut current = self.get(target).and_then(|n| n.parent());
        while let Some(id) = current {
            path.push(id);
            current = self.get(id).and_then(|n| n.parent());
        }
        path.reverse(); // 现在是从 root → ... → target.parent

        for &id in &path {
            if let Some(node) = self.get_mut(id) {
                if node.on_event(event) == EventResult::Handled {
                    return EventResult::Handled;
                }
            }
        }
        EventResult::NotHandled
    }

    fn dispatch_to(&mut self, target: WidgetId, event: &WidgetEvent) -> EventResult {
        // 分发前标记目标为脏，确保事件处理函数（on_event）中的状态变更能被渲染管线感知。
        // 所有事件类型统一在此标记，避免 Timer / FileDrop / Window 等事件类型
        // 在 dispatch_event 中遗漏 mark_dirty 导致状态变更不渲染的问题。
        self.mark_dirty(target);

        let mut current = Some(target);
        // ScrollView 的子节点框架是自然坐标（未含滚动偏移），
        // 必须先计算目标路径上所有 ScrollView 的累计偏移量，
        // 翻译事件后加上该偏移量，使事件坐标与视觉位置一致。
        let scroll_off = self.cumulative_scroll_offset(target);
        while let Some(id) = current {
            let node = match self.get_mut(id) { Some(n) => n, None => return EventResult::NotHandled };
            let frame = node.frame();
            let translated = Self::translate_mouse_event(event, frame);
            let compensated = match scroll_off {
                Some((sx, sy)) => Self::add_offset_to_event(translated, sx, sy),
                None => translated,
            };
            let result = node.on_event(&compensated);
            match result {
                EventResult::Handled => return EventResult::Handled,
                EventResult::Bubbled => { current = node.parent(); }
                EventResult::NotHandled => { current = node.parent(); }
            }
        }
        EventResult::NotHandled
    }

    fn translate_mouse_event(event: &WidgetEvent, frame: Rect) -> WidgetEvent {
        match *event {
            WidgetEvent::MouseDown { pos, button, mods } =>
                WidgetEvent::MouseDown {
                    pos: Point::new(pos.x - frame.x, pos.y - frame.y),
                    button,
                    mods,
                },
            WidgetEvent::MouseUp { pos, button, mods } =>
                WidgetEvent::MouseUp {
                    pos: Point::new(pos.x - frame.x, pos.y - frame.y),
                    button,
                    mods,
                },
            WidgetEvent::MouseMove { pos, mods } =>
                WidgetEvent::MouseMove { pos: Point::new(pos.x - frame.x, pos.y - frame.y), mods },
            ref other => other.clone(),
        }
    }

    fn set_focus(&mut self, new_focus: Option<WidgetId>) {
        if new_focus == self.focused_widget { return; }
        if let Some(old) = self.focused_widget {
            self.mark_dirty(old);
            let _ = self.dispatch_to(old, &WidgetEvent::FocusOut);
        }
        self.focused_widget = new_focus;
        if let Some(new) = new_focus {
            self.mark_dirty(new);
            let _ = self.dispatch_to(new, &WidgetEvent::FocusIn);
        }
    }
}
