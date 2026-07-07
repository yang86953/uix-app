use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::native::traits::input::{KeyCode, KeyMod};
use crate::ui::{ComponentId, EventResult, SystemEvent, WidgetTree};
use std::collections::HashSet;

component! {
    /// Keeps Tab/Shift+Tab focus inside a subtree.
    pub struct FocusTrap {
        /// Whether focus trapping is active.
        active: bool,
        /// Focusable widgets captured after layout.
        focusable_ids: HashSet<ComponentId>,
        /// Offset used while cycling focus.
        tab_offset: usize,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(0.0, 0.0))
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.active {
            return EventResult::NotHandled;
        }
        if let SystemEvent::KeyDown { key, mods } = event {
            if *key == KeyCode::Tab {
                let mut sorted: Vec<ComponentId> = self.focusable_ids.iter().copied().collect();
                sorted.sort();
                if sorted.is_empty() {
                    return EventResult::Handled;
                }
                let shift = mods.contains(KeyMod::SHIFT);
                if shift {
                    self.tab_offset = if self.tab_offset == 0 {
                        sorted.len() - 1
                    } else {
                        self.tab_offset - 1
                    };
                } else {
                    self.tab_offset = (self.tab_offset + 1) % sorted.len();
                }
                return EventResult::Handled;
            }
        }
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
            tab_offset: 0,
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

    pub fn wrap(self, node: crate::ui::WidgetNode) -> crate::ui::WidgetNode {
        crate::ui::WidgetNode::new(Box::new(self), vec![node])
    }
}
