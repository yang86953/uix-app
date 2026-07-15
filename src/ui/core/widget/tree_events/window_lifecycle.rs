use super::*;

impl WidgetTree {
    pub(super) fn dispatch_window_lifecycle(&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::WindowFocus => self.activate_window_focus(),
            SystemEvent::WindowBlur | SystemEvent::WindowMinimize => {
                if let Some(root) = self.root_id {
                    self.cancel_pointer_hover_in_subtree(root);
                }
                self.cancel_active_pointer_gesture();
                self.deactivate_window_focus();
            }
            SystemEvent::WindowMaximize | SystemEvent::WindowRestore => {}
            _ => return EventResult::NotHandled,
        }

        if let Some(root) = self.root_id {
            self.dispatch_to(root, event)
        } else {
            EventResult::NotHandled
        }
    }

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
            )
    }

    pub(super) fn deactivate_window_focus(&mut self) {
        self.keyboard_activation = None;
        if !self.window_focused {
            return;
        }
        self.window_focused = false;
        let Some(focused) = self.managers().focus.focused_component() else {
            return;
        };

        self.invalidate_paint(focused);
        let _ = self.dispatch_to(focused, &SystemEvent::FocusOut);
        for id in self.focus_containment_path(Some(focused)) {
            self.dispatch_focus_within(id, false);
        }
        self.rebuild_widget_overlays();
    }

    pub(super) fn activate_window_focus(&mut self) {
        if self.window_focused {
            return;
        }
        self.window_focused = true;
        let Some(focused) = self
            .managers()
            .focus
            .focused_component()
            .filter(|&id| self.focus_target_available(id))
        else {
            return;
        };

        self.invalidate_paint(focused);
        let _ = self.dispatch_to(focused, &SystemEvent::FocusIn);
        for id in self.focus_containment_path(Some(focused)).into_iter().rev() {
            self.dispatch_focus_within(id, true);
        }
        self.rebuild_widget_overlays();
    }
}
