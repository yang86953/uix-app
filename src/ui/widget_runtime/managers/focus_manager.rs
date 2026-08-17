use std::collections::HashMap;

use crate::ui::ComponentId;

/// 管理组件树的键盘焦点状态和可聚焦组件顺序。
#[derive(Default)]
pub struct FocusManager {
    focused: bool,
    focusable: bool,
    tab_index: i32,
    focused_component: Option<ComponentId>,
    focusable_components: HashMap<ComponentId, i32>,
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
    pub fn focused_component(&self) -> Option<ComponentId> {
        self.focused_component
    }

    /// 设置组件树中当前获得焦点的组件标识。
    pub fn set_focused_component(&mut self, id: Option<ComponentId>) {
        self.focused_component = id;
    }

    /// 按 Tab 导航索引注册组件；非正索引会移除已有注册。
    pub fn register_focusable(&mut self, component_id: ComponentId, tab_index: i32) {
        if tab_index > 0 {
            self.focusable_components.insert(component_id, tab_index);
        } else {
            self.focusable_components.remove(&component_id);
        }
    }

    /// 注销组件，并在该组件持有焦点时清除当前焦点。
    pub fn unregister_component(&mut self, component_id: ComponentId) {
        self.focusable_components.remove(&component_id);
        if self.focused_component == Some(component_id) {
            self.focused_component = None;
        }
    }

    /// 清除组件树的当前焦点及全部可聚焦组件注册。
    pub fn clear_tree_focus(&mut self) {
        self.focused_component = None;
        self.focusable_components.clear();
    }

    /// 返回按 Tab 导航索引及组件标识稳定排序的可聚焦组件。
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
