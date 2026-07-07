use std::collections::HashMap;

use crate::ui::ComponentId;

/// Manages keyboard focus for a component tree.
#[derive(Default)]
pub struct FocusManager {
    focused: bool,
    focusable: bool,
    tab_index: i32,
    focused_component: Option<ComponentId>,
    focusable_components: HashMap<ComponentId, i32>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn focus(&mut self) {
        self.focused = true;
    }
    pub fn blur(&mut self) {
        self.focused = false;
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }
    pub fn is_focusable(&self) -> bool {
        self.focusable
    }

    pub fn set_focusable(&mut self, v: bool) {
        self.focusable = v;
    }

    pub fn tab_index(&self) -> i32 {
        self.tab_index
    }
    pub fn set_tab_index(&mut self, idx: i32) {
        self.tab_index = idx;
    }

    pub fn focused_component(&self) -> Option<ComponentId> {
        self.focused_component
    }

    pub fn set_focused_component(&mut self, id: Option<ComponentId>) {
        self.focused_component = id;
    }

    pub fn register_focusable(&mut self, component_id: ComponentId, tab_index: i32) {
        if tab_index > 0 {
            self.focusable_components.insert(component_id, tab_index);
        } else {
            self.focusable_components.remove(&component_id);
        }
    }

    pub fn unregister_component(&mut self, component_id: ComponentId) {
        self.focusable_components.remove(&component_id);
        if self.focused_component == Some(component_id) {
            self.focused_component = None;
        }
    }

    pub fn clear_tree_focus(&mut self) {
        self.focused_component = None;
        self.focusable_components.clear();
    }

    pub fn focusable_order(&self) -> Vec<ComponentId> {
        let mut focusable: Vec<(i32, ComponentId)> = self
            .focusable_components
            .iter()
            .map(|(&id, &tab_index)| (tab_index, id))
            .collect();
        focusable.sort_by_key(|&(tab_index, id)| (tab_index, id));
        focusable.into_iter().map(|(_, id)| id).collect()
    }
}
