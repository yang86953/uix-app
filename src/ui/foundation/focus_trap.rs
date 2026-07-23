use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::api::PaintContext;
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
    let edge = if forward { 0 } else { focusable.len() - 1 };
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
            None => Some(focusable[edge]),
        }
    } else {
        Some(focusable[edge])
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

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        children.iter().map(|child| (child.id, frame)).collect()
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

    pub(crate) fn is_active(&self) -> bool {
        self.active
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
