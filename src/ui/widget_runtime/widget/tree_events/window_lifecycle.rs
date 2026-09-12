use super::*;

impl WidgetTree {
    /// 分发窗口生命周期事件（焦点得失、最小化、最大化等）。
    pub(super) fn dispatch_window_lifecycle(&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::WindowFocus => self.activate_window_focus(),
            // 窗口失焦或最小化：取消悬浮与手势，并注销窗口焦点。
            SystemEvent::WindowBlur | SystemEvent::WindowMinimize => {
                if let Some(root) = self.root_id {
                    self.cancel_pointer_hover_in_subtree(root);
                }
                self.cancel_active_pointer_gesture();
                self.deactivate_window_focus();
            }
            // 最大化/还原不改变焦点语义，仅继续向下分发。
            SystemEvent::WindowMaximize | SystemEvent::WindowRestore => {}
            _ => return EventResult::NotHandled,
        }

        // 生命周期事件仍需沿树向下分发，便于各组件自行响应。
        if let Some(root) = self.root_id {
            self.dispatch_to(root, event)
        } else {
            EventResult::NotHandled
        }
    }

    /// 窗口未聚焦时，这些交互类事件是否应被忽略。
    ///
    /// 指针事件与全部主流桌面语义保持一致：未聚焦窗口照常接收悬停与点击
    /// （Wayland 指针输入独立于键盘焦点，Windows/macOS 对非激活窗口同样
    /// 投递 hover），悬停与点击不要求窗口先获得焦点。键盘、IME、剪贴板与
    /// 文件拖放本质上依赖键盘焦点，未聚焦时仍被忽略。
    pub(super) fn ignores_input_while_window_unfocused(&self, event: &SystemEvent) -> bool {
        !self.window_focused
            && matches!(
                event,
                SystemEvent::KeyDown { .. }
                    | SystemEvent::KeyUp { .. }
                    | SystemEvent::TextInput { .. }
                    | SystemEvent::ImeCompositionStart
                    | SystemEvent::ImeCompositionUpdate { .. }
                    | SystemEvent::ImeCompositionEnd { .. }
                    | SystemEvent::Copy
                    | SystemEvent::Cut
                    | SystemEvent::Paste { .. }
                    | SystemEvent::FileDrop { .. }
            )
    }

    /// 注销窗口焦点：清除键盘激活目标，并向聚焦组件下发 FocusOut。
    pub(super) fn deactivate_window_focus(&mut self) {
        self.keyboard_activation = None;
        // 窗口焦点本已注销则直接返回。
        if !self.window_focused {
            return;
        }
        self.window_focused = false;
        // 窗口失焦即键盘焦点不可见，同步本树响应式事实。
        self.widget_state_store.sync_keyboard_focus_visible_fact(false);
        // 无聚焦组件时无需下发失焦事件。
        let Some(focused) = self.managers().focus.focused_widget() else {
            return;
        };

        // 使聚焦组件重绘、下发 FocusOut，并沿包含路径逐层取消 focus-within。
        self.invalidate_paint(focused);
        let _ = self.dispatch_to(focused, &SystemEvent::FocusOut);
        for id in self.focus_containment_path(Some(focused)) {
            self.dispatch_focus_within(id, false);
        }
        self.rebuild_widget_overlays();
    }

    /// 注册窗口焦点：向仍可聚焦的组件下发 FocusIn 并重绘。
    pub(super) fn activate_window_focus(&mut self) {
        // 窗口焦点已注册则直接返回。
        if self.window_focused {
            return;
        }
        self.window_focused = true;
        // 恢复窗口聚焦后按键盘可见标志同步本树响应式事实。
        self.widget_state_store
            .sync_keyboard_focus_visible_fact(self.keyboard_focus_visible());
        // 聚焦组件需仍然可聚焦（如未被禁用），否则跳过激活。
        let Some(focused) = self
            .managers()
            .focus
            .focused_widget()
            .filter(|&id| self.focus_target_available(id))
        else {
            return;
        };

        // 使聚焦组件重绘、下发 FocusIn，并沿包含路径自外向内逐层设置 focus-within。
        self.invalidate_paint(focused);
        let _ = self.dispatch_to(focused, &SystemEvent::FocusIn);
        for id in self.focus_containment_path(Some(focused)).into_iter().rev() {
            self.dispatch_focus_within(id, true);
        }
        self.rebuild_widget_overlays();
    }
}
