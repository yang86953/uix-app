use std::collections::HashMap;

use crate::ui::ComponentId;

/// Manages keyboard focus for a widget tree.
#[derive(Default)]
pub struct FocusManager {
    focused: bool,
    focusable: bool,
    tab_index: i32,
    focused_widget: Option<ComponentId>,
    focusable_widgets: HashMap<ComponentId, i32>,
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

    pub fn focused_widget(&self) -> Option<ComponentId> {
        self.focused_widget
    }

    pub fn set_focused_widget(&mut self, id: Option<ComponentId>) {
        self.focused_widget = id;
    }

    pub fn register_focusable(&mut self, component_id: ComponentId, tab_index: i32) {
        if tab_index > 0 {
            self.focusable_widgets.insert(component_id, tab_index);
        } else {
            self.focusable_widgets.remove(&component_id);
        }
    }

    pub fn unregister_widget(&mut self, component_id: ComponentId) {
        self.focusable_widgets.remove(&component_id);
        if self.focused_widget == Some(component_id) {
            self.focused_widget = None;
        }
    }

    pub fn clear_tree_focus(&mut self) {
        self.focused_widget = None;
        self.focusable_widgets.clear();
    }

    pub fn focusable_order(&self) -> Vec<ComponentId> {
        let mut focusable: Vec<(i32, ComponentId)> = self
            .focusable_widgets
            .iter()
            .map(|(&id, &tab_index)| (tab_index, id))
            .collect();
        focusable.sort_by_key(|&(tab_index, id)| (tab_index, id));
        focusable.into_iter().map(|(_, id)| id).collect()
    }
}
