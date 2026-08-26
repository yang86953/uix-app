use std::collections::HashMap;

use crate::ui::WidgetId;

/// 管理组件树的键盘焦点状态和可聚焦组件顺序。
#[derive(Default)]
pub struct FocusManager {
    focused: bool,
    focusable: bool,
    tab_index: i32,
    focused_widget: Option<WidgetId>,
    focusable_widgets: HashMap<WidgetId, i32>,
}

impl FocusManager {
    /// 创建没有当前焦点且未注册可聚焦组件的管理器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 将管理器所属组件标记为已获得焦点。
    pub fn focus(&mut self) {
        self.focused = true;
    }
    /// 将管理器所属组件标记为已失去焦点。
    pub fn blur(&mut self) {
        self.focused = false;
    }

    /// 返回管理器所属组件当前是否已获得焦点。
    pub fn is_focused(&self) -> bool {
        self.focused
    }
    /// 返回管理器所属组件是否允许获得焦点。
    pub fn is_focusable(&self) -> bool {
        self.focusable
    }

    /// 设置管理器所属组件是否允许获得焦点。
    pub fn set_focusable(&mut self, v: bool) {
        self.focusable = v;
    }

    /// 返回管理器所属组件的 Tab 导航索引。
    pub fn tab_index(&self) -> i32 {
        self.tab_index
    }
    /// 设置管理器所属组件的 Tab 导航索引。
    pub fn set_tab_index(&mut self, idx: i32) {
        self.tab_index = idx;
    }

    /// 返回组件树中当前获得焦点的组件标识。
    pub fn focused_widget(&self) -> Option<WidgetId> {
        self.focused_widget
    }

    /// 设置组件树中当前获得焦点的组件标识。
    pub fn set_focused_widget(&mut self, id: Option<WidgetId>) {
        self.focused_widget = id;
    }

    /// 按 Tab 导航索引注册组件；非正索引会移除已有注册。
    pub fn register_focusable(&mut self, widget_id: WidgetId, tab_index: i32) {
        if tab_index > 0 {
            self.focusable_widgets.insert(widget_id, tab_index);
        } else if !self.focusable_widgets.is_empty() {
            // 空注册表不可能持有当前节点，避免为常见非聚焦节点计算 WidgetId 哈希。
            self.focusable_widgets.remove(&widget_id);
        }
    }

    /// 注销组件，并在该组件持有焦点时清除当前焦点。
    pub fn unregister_widget(&mut self, widget_id: WidgetId) {
        if !self.focusable_widgets.is_empty() {
            // 空注册表无需执行带哈希的删除，焦点清理仍独立遵循节点身份。
            self.focusable_widgets.remove(&widget_id);
        }
        if self.focused_widget == Some(widget_id) {
            self.focused_widget = None;
        }
    }

    /// 清除组件树的当前焦点及全部可聚焦组件注册。
    pub fn clear_tree_focus(&mut self) {
        self.focused_widget = None;
        self.focusable_widgets.clear();
    }

    /// 返回按 Tab 导航索引及组件标识稳定排序的可聚焦组件。
    pub fn focusable_order(&self) -> Vec<WidgetId> {
        let mut focusable: Vec<(i32, WidgetId)> = self
            .focusable_widgets
            .iter()
            .map(|(&id, &tab_index)| (tab_index, id))
            .collect();
        focusable.sort_by_key(|&(tab_index, id)| (tab_index, id));
        focusable.into_iter().map(|(_, id)| id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry_removal_preserves_focus_and_registration_semantics() {
        let mut manager = FocusManager::new();
        let first = WidgetId::new(1);
        let second = WidgetId::new(2);

        manager.register_focusable(first, 0);
        assert!(manager.focusable_order().is_empty());

        manager.register_focusable(first, 3);
        manager.register_focusable(second, 1);
        assert_eq!(manager.focusable_order(), vec![second, first]);

        manager.register_focusable(first, 0);
        assert_eq!(manager.focusable_order(), vec![second]);

        manager.set_focused_widget(first.into());
        manager.unregister_widget(first);
        assert_eq!(manager.focused_widget(), None);

        manager.unregister_widget(second);
        manager.unregister_widget(second);
        assert!(manager.focusable_order().is_empty());
    }
}
