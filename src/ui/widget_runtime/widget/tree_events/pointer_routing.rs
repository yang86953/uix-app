use super::*;
use crate::ui::event::SemanticKind;
use crate::ui::{OverlayEntry, OverlayKind};

impl WidgetTree {
    pub(crate) fn cancel_hidden_interaction(&mut self) {
        let targets = [
            self.managers().interaction.hovered_widget(),
            self.managers().interaction.pressed_widget(),
            self.managers().drag.target(),
        ];
        let mut cancelled = Vec::new();
        // manager 目标通常相邻重复，保留最近一次成功结果即可覆盖稳态且不增加扫描成本。
        let mut last_visible = None;
        for target in targets.into_iter().flatten() {
            if cancelled.contains(&target) || last_visible == Some(target) {
                continue;
            }
            if !self.is_effectively_visible(target) {
                self.cancel_pointer_state_in_subtree(target);
                cancelled.push(target);
                // 取消回调可能修改任意组件状态，后续目标必须重新验证。
                last_visible = None;
                continue;
            }
            last_visible = Some(target);
        }
        if let Some(focused) = self.managers().focus.focused_widget() {
            let visible = last_visible == Some(focused) || self.is_effectively_visible(focused);
            if !visible || !self.focus_target_interactive(focused) {
                self.set_focus(None);
            }
        }
    }

    pub(crate) fn cancel_subtree_interaction(&mut self, root: WidgetId) {
        self.cancel_pointer_state_in_subtree(root);
        if self
            .managers()
            .focus
            .focused_widget()
            .is_some_and(|focused| self.is_descendant_of(focused, root))
        {
            self.set_focus(None);
        }
    }

    pub(crate) fn cancel_pointer_state_in_subtree(&mut self, root: WidgetId) {
        self.cancel_pointer_hover_in_subtree(root);
        self.cancel_pointer_gesture_in_subtree(root);
    }

    pub(crate) fn cancel_pointer_hover_in_subtree(&mut self, root: WidgetId) {
        let hovered = self
            .managers()
            .interaction
            .hovered_widget()
            .filter(|target| self.is_descendant_of(*target, root));
        let Some(hovered) = hovered else {
            return;
        };
        let leave_is_delivered_by_gesture_cancel =
            self.managers().interaction.pressed_widget() == Some(hovered);
        self.managers_mut().interaction.set_hovered_widget(None);
        if !leave_is_delivered_by_gesture_cancel
            && self.dispatch_to(hovered, &SystemEvent::PointerLeave) == EventResult::Handled
        {
            self.invalidate_paint(hovered);
        }
        self.rebuild_widget_overlays();
    }

    pub(crate) fn cancel_pointer_gesture_in_subtree(&mut self, root: WidgetId) {
        let owns_pressed = self
            .managers()
            .interaction
            .pressed_widget()
            .is_some_and(|target| self.is_descendant_of(target, root));
        let owns_drag = self
            .managers()
            .drag
            .target()
            .is_some_and(|target| self.is_descendant_of(target, root));
        if owns_pressed || owns_drag {
            self.cancel_active_pointer_gesture();
        }
    }

    pub(super) fn cancel_active_pointer_gesture(&mut self) {
        let pressed = self.managers().interaction.pressed_widget();
        let drag = self.managers().drag.is_dragging().then(|| {
            (
                self.managers().drag.target(),
                self.managers().drag.last_pos(),
                self.managers().drag.button(),
                self.managers().drag.mods(),
            )
        });
        self.managers_mut().interaction.set_pressed_widget(None);
        self.managers_mut().drag.end_drag();

        if let Some((Some(target), pos, button, mods)) = drag {
            // DragEnd 未被拖拽目标消费：记录日志定位路由失败，行为不变。
            if self.dispatch_to(target, &SystemEvent::DragEnd { pos, button, mods })
                == EventResult::NotHandled
            {
                tracing::warn!(event = "DragEnd", target = ?target, "drag end was not handled");
            }
        }
        if let Some(pressed) = pressed {
            self.invalidate_paint(pressed);
            // PointerLeave 是可选生命周期通知；无状态组件不消费时仍保持真实返回值。
            let _ = self.dispatch_to(pressed, &SystemEvent::PointerLeave);
        }
        self.rebuild_widget_overlays();
    }

    // 原生窗口管理器接管指针后，清除 UI pressed/drag 而不改变键盘焦点。
    pub(crate) fn cancel_pointer_gesture_for_native_handoff(&mut self) {
        // 复用唯一手势取消实现，统一交付 DragEnd 与 PointerLeave 清理语义。
        self.cancel_active_pointer_gesture();
        // 结束原生指针接管清理。
    }

    pub(super) fn dispatch_pointer_release(
        &mut self,
        event: &SystemEvent,
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    ) -> EventResult {
        let hold = self
            .managers_mut()
            .interaction
            .release_pressed_pointer(button);
        let releases_drag = self.managers().drag.is_gesture_button(button);
        // 仅由启动键结束拖拽；其他按键释放不改变 potential/active 手势。
        if releases_drag && self.managers().drag.is_dragging() {
            if let Some(target) = self.managers().drag.target() {
                let drag_end = SystemEvent::DragEnd { pos, button, mods };
                // DragEnd 未被拖拽目标消费：记录日志，行为不变。
                if self.dispatch_to(target, &drag_end) == EventResult::NotHandled {
                    tracing::warn!(event = "DragEnd", target = ?target, "drag end was not handled");
                }
            }
        }
        if releases_drag {
            self.managers_mut().drag.end_drag();
        }
        let hit = self.pointer_target_at(pos);
        let mut result = EventResult::NotHandled;
        if let Some(target) = hit {
            self.invalidate_paint(target);
            let actions_before_dispatch = self.pending_window_actions.len();
            if self.capture_to(target, event).is_some() {
                if let Some(pressed) = hold {
                    self.invalidate_paint(pressed);
                    // 捕获接管只需交付离开通知；零消费者是合法状态。
                    let _ = self.dispatch_to(pressed, &SystemEvent::PointerLeave);
                }
                self.rebuild_widget_overlays();
                return EventResult::Handled;
            }
            result = self.dispatch_to(target, event);
            let title_bar_system_menu = self.pending_window_actions[actions_before_dispatch..]
                .contains(&WindowAction::ShowSystemMenuFromTitleBar);
            // 只有同一目标、同一鼠标键的 down/up 才合成 Click。
            if hold == Some(target) && !title_bar_system_menu {
                let click = ClickEvent {
                    button,
                    pos,
                    modifiers: mods,
                };
                // Click 是可选观察事件；零订阅者必须安静地保留 NotHandled 契约。
                let _ = self.dispatch_semantic(SemanticEvent::click(target, click));
                if button == MouseButton::Right {
                    let mut context_menu = SemanticEvent::context_menu(target, click);
                    // ContextMenu 语义未被消费：记录日志，行为不变。
                    if self.dispatch_semantic_event(&mut context_menu) == EventResult::NotHandled {
                        tracing::warn!(
                            event = "ContextMenu",
                            target = ?target,
                            "context menu semantic was not handled"
                        );
                    }
                    if !context_menu.default_prevented() {
                        self.open_context_menu_overlay(target, click.pos);
                    }
                }
            }
        }
        if let Some(pressed) = hold {
            if Some(pressed) != hit {
                self.invalidate_paint(pressed);
                // 捕获目标外松开时，先通知指针已离开，再交付最终 PointerUp。
                // 离开通知没有消费者属于正常情况，不提升为路由失败。
                let _ = self.dispatch_to(pressed, &SystemEvent::PointerLeave);
                // PointerUp 是已建立手势的最终命令，未被消费仍保留诊断。
                if self.dispatch_to(pressed, event) == EventResult::NotHandled {
                    tracing::warn!(event = "PointerUp", target = ?pressed, "pointer up was not handled");
                }
            }
        }
        self.rebuild_widget_overlays();
        result
    }

    pub(super) fn overlay_target_at(&self, pos: Point) -> Option<WidgetId> {
        let owner = self
            .overlay_stack
            .hit_test(pos.x, pos.y)
            .map(|entry| entry.owner())?;
        self.hit_test_internal(owner, pos).or(Some(owner))
    }

    pub(crate) fn pointer_target_at(&self, pos: Point) -> Option<WidgetId> {
        self.overlay_target_at(pos).or_else(|| self.hit_test(pos))
    }

    fn blocking_top_overlay_at(&self, pos: Point) -> Option<&OverlayEntry> {
        let top = self.overlay_stack.top()?;
        let inside_top = top.bounds_rect().is_some_and(|bounds| bounds.contains(pos));
        (!inside_top && (top.is_modal() || top.traps_focus() || top.dismisses_on_outside()))
            .then_some(top)
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn pointer_down_blocker_at(&self, pos: Point) -> Option<WidgetId> {
        self.blocking_top_overlay_at(pos).map(OverlayEntry::owner)
    }

    pub(super) fn intercept_top_overlay_outside_pointer_down(
        &mut self,
        pos: Point,
    ) -> Option<EventResult> {
        let top = self.blocking_top_overlay_at(pos).cloned()?;
        if top.dismisses_on_outside() {
            // 先通知具体 owner 执行用户取消语义，再移除当前浮层登记。
            crate::ui::tree_widget_hooks::dismiss_overlay_owner_from_outside(self, top.owner());
            self.overlay_stack.remove(top.id());
            self.invalidate_paint(top.owner());
        }
        self.restore_focus_after_trap_owner(top.owner());
        Some(EventResult::Handled)
    }

    pub(super) fn open_context_menu_overlay(&mut self, owner: WidgetId, pos: Point) {
        self.overlay_stack
            .retain_entries(|entry| entry.kind() != OverlayKind::ContextMenu);
        self.overlay_stack.push_entry(
            OverlayEntry::new(owner, OverlayKind::ContextMenu)
                .bounds(Rect::new(pos.x, pos.y, 160.0, 160.0))
                .z_index(1200)
                .dismiss_on_outside(true)
                .managed(true),
        );
        self.invalidate_paint(owner);
    }

    pub(super) fn secondary_pointer_drag_boundary(
        &self,
        target: WidgetId,
        event: &SystemEvent,
    ) -> Option<WidgetId> {
        if !matches!(
            event,
            SystemEvent::PointerDown {
                button: MouseButton::Right,
                ..
            }
        ) {
            return None;
        }

        let mut current = Some(target);
        let mut reserves_context_menu = false;
        while let Some(id) = current {
            let node = self.get(id)?;
            if crate::ui::tree_widget_hooks::is_drag_region(self, id) {
                return reserves_context_menu.then_some(id);
            }
            reserves_context_menu |= node.tab_index() > 0
                || node
                    .handler_signatures()
                    .iter()
                    .any(|signature| signature.kind == SemanticKind::ContextMenu);
            current = node.parent();
        }
        None
    }
}

// 单元测试验证原生窗口接管只清理指针手势而保留键盘焦点。
#[cfg(test)]
// 测试模块直接观察 WidgetTree manager 状态，不依赖真实窗口。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widget_runtime/widget/tree_events/pointer_routing__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
