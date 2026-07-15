use super::*;
use crate::ui::event::SemanticKind;
use crate::ui::window_chrome::WindowInteractionRegion;
use crate::ui::{OverlayEntry, OverlayKind};

impl WidgetTree {
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
        // 如果拖拽处于活跃状态，发射 DragEnd 到拖拽目标。
        if self.managers().drag.is_dragging() {
            if let Some(target) = self.managers().drag.target() {
                let drag_end = SystemEvent::DragEnd { pos, button, mods };
                let _ = self.dispatch_to(target, &drag_end);
            }
        }
        self.managers_mut().drag.end_drag();
        let hit = self.overlay_target_at(pos).or_else(|| self.hit_test(pos));
        let mut result = EventResult::NotHandled;
        if let Some(target) = hit {
            self.invalidate_paint(target);
            let actions_before_dispatch = self.pending_window_actions.len();
            if self.capture_to(target, event) == EventResult::Handled {
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
                let _ = self.dispatch_semantic(SemanticEvent::click(target, click));
                if button == MouseButton::Right {
                    let mut context_menu = SemanticEvent::context_menu(target, click);
                    let _ = self.dispatch_semantic_event(&mut context_menu);
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
                let _ = self.dispatch_to(pressed, &SystemEvent::PointerLeave);
                let _ = self.dispatch_to(pressed, event);
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

    pub(super) fn intercept_top_overlay_outside_pointer_down(
        &mut self,
        pos: Point,
    ) -> Option<EventResult> {
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
            self.restore_focus_after_trap_owner(top.owner());
            return Some(EventResult::Handled);
        }

        None
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
            if node
                .component()
                .as_any()
                .downcast_ref::<WindowInteractionRegion>()
                .is_some_and(WindowInteractionRegion::is_drag_region)
            {
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
