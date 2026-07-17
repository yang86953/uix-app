use super::*;

impl WidgetTree {
    fn is_keyboard_activation_key(key: KeyCode) -> bool {
        matches!(key, KeyCode::Enter | KeyCode::Space)
    }

    pub(super) fn dispatch_key_down(
        &mut self,
        event: &SystemEvent,
        key: KeyCode,
        mods: KeyMod,
    ) -> EventResult {
        // Tab 键焦点导航（在捕获和冒泡之前处理）。
        if key == KeyCode::Tab {
            let forward = !mods.contains(KeyMod::SHIFT);
            if let Some(owner) = self
                .overlay_stack
                .top()
                .filter(|entry| entry.traps_focus())
                .map(|entry| entry.owner())
            {
                if let Some(next) = self.focus_next_in_scope(owner, forward) {
                    self.remember_focus_before_trap(owner);
                    self.set_focus(Some(next));
                    return EventResult::Handled;
                }
                return EventResult::NotHandled;
            }
            if let Some(owner) = self
                .managers()
                .focus
                .focused_component()
                .and_then(|focused| self.active_focus_trap_ancestor(focused))
            {
                if let Some(next) = self.focus_next_in_scope(owner, forward) {
                    self.set_focus(Some(next));
                    return EventResult::Handled;
                }
                return EventResult::NotHandled;
            }
            if let Some(next) = self.focus_next(forward) {
                self.set_focus(Some(next));
                return EventResult::Handled;
            }
            return EventResult::NotHandled;
        }

        // Escape is owned by the top overlay even when focus is still on the
        // trigger behind it (for example a Drawer without an initial child).
        // Route dismissal before the normal focused-node path so modal
        // overlays cannot become keyboard-inaccessible.
        if key == KeyCode::Escape {
            if let Some(owner) = self.overlay_stack.top().map(|entry| entry.owner()) {
                if self.dispatch_to(owner, event) == EventResult::Handled {
                    return EventResult::Handled;
                }
            }
        }

        let Some(target) = self.managers().focus.focused_component() else {
            return EventResult::NotHandled;
        };
        // 跨节点文字选区：Ctrl+C 须聚合兄弟选区，不能只读焦点节点。
        if key == KeyCode::C
            && mods.contains(KeyMod::CTRL)
            && self.try_copy_cross_text_selection(target)
        {
            return EventResult::Handled;
        }
        if Self::is_keyboard_activation_key(key) && self.keyboard_activation.is_some() {
            // 自动重复或另一激活键不能重复启动、替换当前键盘手势。
            return EventResult::Handled;
        }
        // 捕获阶段：root → target，用于全局快捷键。
        if self.capture_to(target, event) == EventResult::Handled {
            return EventResult::Handled;
        }
        let result = self.dispatch_to(target, event);
        if result == EventResult::Handled && Self::is_keyboard_activation_key(key) {
            self.keyboard_activation = Some((target, key, mods));
        }
        result
    }

    fn active_focus_trap_ancestor(&self, target: WidgetId) -> Option<WidgetId> {
        let mut current = Some(target);
        while let Some(id) = current {
            let node = self.get(id)?;
            if node
                .component()
                .as_any()
                .downcast_ref::<crate::ui::foundation::FocusTrap>()
                .is_some_and(crate::ui::foundation::FocusTrap::is_active)
            {
                return Some(id);
            }
            current = node.parent();
        }
        None
    }

    pub(super) fn dispatch_key_up(&mut self, event: &SystemEvent, key: KeyCode) -> EventResult {
        if Self::is_keyboard_activation_key(key) {
            let Some((target, armed_key, armed_mods)) = self.keyboard_activation else {
                return self.dispatch_unarmed_key_up(event);
            };
            if key != armed_key {
                // 其他激活键的释放不能完成或取消当前手势。
                return EventResult::Handled;
            }
            self.keyboard_activation = None;
            if self.managers().focus.focused_component() != Some(target)
                || !self.focus_target_available(target)
            {
                return EventResult::NotHandled;
            }

            self.invalidate_paint(target);
            // 已接受的 KeyDown 锁定目标；匹配 KeyUp 必须回到同一目标完成释放。
            let _ = self.dispatch_to(target, event);
            let click = ClickEvent {
                button: MouseButton::Left,
                pos: self
                    .get(target)
                    .map(|node| {
                        let frame = node.frame();
                        Point::new(frame.x + frame.w * 0.5, frame.y + frame.h * 0.5)
                    })
                    .unwrap_or_default(),
                modifiers: armed_mods,
            };
            let _ = self.dispatch_semantic(SemanticEvent::click(target, click));
            return EventResult::Handled;
        }

        self.dispatch_unarmed_key_up(event)
    }

    fn dispatch_unarmed_key_up(&mut self, event: &SystemEvent) -> EventResult {
        let Some(target) = self.managers().focus.focused_component() else {
            return EventResult::NotHandled;
        };
        self.invalidate_paint(target);
        // 捕获阶段：root → target。
        if self.capture_to(target, event) == EventResult::Handled {
            return EventResult::Handled;
        }
        self.dispatch_to(target, event)
    }
}
