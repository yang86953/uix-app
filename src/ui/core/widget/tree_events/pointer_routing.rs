use super::*;
use crate::ui::event::SemanticKind;
use crate::ui::window_chrome::WindowInteractionRegion;
use crate::ui::{OverlayEntry, OverlayKind};

impl WidgetTree {
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
