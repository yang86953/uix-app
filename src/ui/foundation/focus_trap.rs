use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::{ComponentId, EventResult, SystemEvent, WidgetTree};
use std::collections::HashSet;

pub fn next_focus_in_order(
    focusable: &[ComponentId],
    current: Option<ComponentId>,
    forward: bool,
) -> Option<ComponentId> {
    if focusable.is_empty() {
        return None;
    }
    if let Some(cur_id) = current {
        let pos = focusable.iter().position(|&id| id == cur_id);
        match pos {
            Some(p) => {
                if forward {
                    Some(focusable[(p + 1) % focusable.len()])
                } else {
                    Some(focusable[(p + focusable.len() - 1) % focusable.len()])
                }
            }
            None => Some(focusable[0]),
        }
    } else {
        Some(focusable[0])
    }
}

component! {
    /// Dispatch-managed focus trap metadata and cycling helper.
    pub struct FocusTrap {
        /// Whether focus trapping is active.
        active: bool,
        /// Focusable widgets captured after layout.
        focusable_ids: HashSet<ComponentId>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(0.0, 0.0))
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}

    on_event => (&mut self, _event: &SystemEvent) -> EventResult {
        EventResult::NotHandled
    }

}

impl Default for FocusTrap {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusTrap {
    pub fn new() -> Self {
        Self {
            active: true,
            focusable_ids: HashSet::new(),
        }
    }

    pub fn active(mut self, v: bool) -> Self {
        self.active = v;
        self
    }

    /// Updates the focusable widget list after layout.
    pub fn update_focusable(&mut self, ids: HashSet<ComponentId>) {
        self.focusable_ids = ids;
    }

    pub fn next_focus(&self, current: Option<ComponentId>, forward: bool) -> Option<ComponentId> {
        if !self.active {
            return None;
        }
        let mut sorted: Vec<ComponentId> = self.focusable_ids.iter().copied().collect();
        sorted.sort();
        next_focus_in_order(&sorted, current, forward)
    }

    pub fn wrap(
        self,
        node: crate::ui::core::widget::WidgetNode,
    ) -> crate::ui::core::widget::WidgetNode {
        crate::ui::core::widget::WidgetNode::new(Box::new(self), vec![node])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::input::{KeyCode, KeyMod};
    use crate::ui::traits::EventHandler;

    #[test]
    fn next_focus_in_order_wraps_forward_and_backward() {
        let ids = [
            ComponentId::new(1),
            ComponentId::new(3),
            ComponentId::new(5),
        ];

        assert_eq!(
            next_focus_in_order(&ids, Some(ComponentId::new(1)), true),
            Some(ComponentId::new(3))
        );
        assert_eq!(
            next_focus_in_order(&ids, Some(ComponentId::new(5)), true),
            Some(ComponentId::new(1))
        );
        assert_eq!(
            next_focus_in_order(&ids, Some(ComponentId::new(1)), false),
            Some(ComponentId::new(5))
        );
        assert_eq!(
            next_focus_in_order(&ids, Some(ComponentId::new(9)), true),
            Some(ComponentId::new(1))
        );
    }

    #[test]
    fn focus_trap_component_does_not_intercept_tab_without_tree_scope() {
        let mut trap = FocusTrap::new();

        assert_eq!(
            trap.on_event(&SystemEvent::KeyDown {
                key: KeyCode::Tab,
                mods: KeyMod::NONE,
            }),
            EventResult::NotHandled
        );
    }

    #[test]
    fn focus_trap_next_focus_uses_sorted_focusable_ids() {
        let mut ids = HashSet::new();
        ids.insert(ComponentId::new(8));
        ids.insert(ComponentId::new(2));
        ids.insert(ComponentId::new(5));
        let mut trap = FocusTrap::new();
        trap.update_focusable(ids);

        assert_eq!(
            trap.next_focus(Some(ComponentId::new(2)), true),
            Some(ComponentId::new(5))
        );
        assert_eq!(
            trap.active(false)
                .next_focus(Some(ComponentId::new(2)), true),
            None
        );
    }
}
