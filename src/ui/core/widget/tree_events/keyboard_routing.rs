use super::*;

impl WidgetTree {
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
            if let Some(next) = self.focus_next(forward) {
                self.set_focus(Some(next));
                return EventResult::Handled;
            }
            return EventResult::NotHandled;
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
        // 捕获阶段：root → target，用于全局快捷键。
        if self.capture_to(target, event) == EventResult::Handled {
            return EventResult::Handled;
        }
        let result = self.dispatch_to(target, event);
        if result == EventResult::Handled && matches!(key, KeyCode::Enter | KeyCode::Space) {
            let click = ClickEvent {
                button: MouseButton::Left,
                pos: self
                    .get(target)
                    .map(|node| {
                        let frame = node.frame();
                        Point::new(frame.x + frame.w * 0.5, frame.y + frame.h * 0.5)
                    })
                    .unwrap_or_default(),
                modifiers: mods,
            };
            let _ = self.dispatch_semantic(SemanticEvent::click(target, click));
        }
        result
    }

    pub(super) fn dispatch_key_up(&mut self, event: &SystemEvent) -> EventResult {
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
