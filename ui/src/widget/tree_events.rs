use super::tree_core::WidgetTree;
use super::*;

impl WidgetTree {
    pub fn hit_test(&self, pos: Point) -> Option<WidgetId> {
        self.root_id.and_then(|root| self.hit_test_internal(root, pos))
    }

    fn hit_test_internal(&self, id: WidgetId, pos: Point) -> Option<WidgetId> {
        let node = self.get(id)?;
        if !node.visible() { return None; }
        let mut sorted: Vec<WidgetId> = node.children().to_vec();
        sorted.sort_by(|&a, &b| {
            let za = self.get(a).map_or(0, |c| c.z_index());
            let zb = self.get(b).map_or(0, |c| c.z_index());
            zb.cmp(&za)
        });
        for &child_id in &sorted {
            if let Some(hit) = self.hit_test_internal(child_id, pos) { return Some(hit); }
        }
        if node.frame().contains(pos) { Some(id) } else { None }
    }

    pub fn dispatch_event(&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                let target = self.hit_test(*pos);
                self.mouse_down_target = target;
                if let Some(t) = target {
                    self.mark_dirty(t);
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled { self.set_focus(Some(t)); }
                    result
                } else { self.set_focus(None); EventResult::NotHandled }
            }
            WidgetEvent::MouseUp { pos, .. } => {
                let hold = self.mouse_down_target;
                self.mouse_down_target = None;
                let hit = self.hit_test(*pos);
                let mut result = EventResult::NotHandled;
                if let Some(t) = hit { self.mark_dirty(t); result = self.dispatch_to(t, event); }
                if let Some(t) = hold {
                    if Some(t) != hit { self.mark_dirty(t); let _ = self.dispatch_to(t, event); }
                }
                result
            }
            WidgetEvent::MouseMove { pos } => {
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
                if let Some(t) = new_hover { self.dispatch_to(t, event) }
                else { EventResult::NotHandled }
            }
            WidgetEvent::MouseWheel { pos, .. } => {
                let target = self.hit_test(*pos).or(self.hovered_widget).or(self.root_id);
                if let Some(t) = target { self.mark_dirty(t); self.dispatch_to(t, event) }
                else { EventResult::NotHandled }
            }
            WidgetEvent::KeyDown { .. } | WidgetEvent::KeyUp { .. } | WidgetEvent::KeyPress { .. } => {
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

    fn dispatch_to(&mut self, target: WidgetId, event: &WidgetEvent) -> EventResult {
        let mut current = Some(target);
        while let Some(id) = current {
            let node = match self.get_mut(id) { Some(n) => n, None => return EventResult::NotHandled };
            let frame = node.frame();
            let translated = Self::translate_mouse_event(event, frame);
            let result = node.inner_mut().on_event(&translated);
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
            WidgetEvent::MouseMove { pos } =>
                WidgetEvent::MouseMove { pos: Point::new(pos.x - frame.x, pos.y - frame.y) },
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
