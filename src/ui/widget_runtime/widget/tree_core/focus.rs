use super::*;
use crate::ui::event::HandlerTable;
use crate::ui::widget_runtime::focus_handle::FocusHandle;
use crate::ui::widget_runtime::focus_trap::next_focus_in_order;

impl WidgetTree {
    /// 聚焦遍历中首个指定组件类型的节点，并返回其身份。
    pub fn focus_by_type<T: Widget + 'static>(&mut self) -> Option<WidgetId> {
        let id = self.find_by_type::<T>()?;
        self.set_focus(Some(id));
        Some(id)
    }

    /// 返回当前焦点节点是否属于指定组件类型。
    pub fn is_focused_type<T: Widget + 'static>(&self) -> bool {
        self.managers
            .focus
            .focused_widget()
            .and_then(|id| self.get(id))
            .map(|node| node.widget().as_any().downcast_ref::<T>().is_some())
            .unwrap_or(false)
    }

    /// 返回事件处理器表的可变借用。
    pub fn handler_table(&mut self) -> &mut HandlerTable {
        // 停止树不得向外暴露可重新安装用户闭包的 sidecar。
        assert!(self.accepts_coordination_work());
        &mut self.handler_table
    }

    pub(crate) fn replace_render_handlers(
        &mut self,
        id: WidgetId,
        handlers: Vec<RenderHandlerRegistration>,
    ) {
        // 停止树不得重新持有延迟渲染闭包。
        assert!(self.accepts_coordination_work());
        self.render_handler_table.replace_widget(id, handlers);
    }

    pub(crate) fn replace_system_event_handlers(
        &mut self,
        id: WidgetId,
        handlers: Vec<crate::ui::event::system_event_handler::SystemEventHandlerRegistration>,
    ) {
        // 停止树不得重新持有系统事件闭包。
        assert!(self.accepts_coordination_work());
        if let Some(node) = self.get_mut(id) {
            node.replace_system_event_handlers(handlers);
        }
    }

    pub(crate) fn set_tab_index_override(&mut self, id: WidgetId, tab_index: Option<i32>) {
        if let Some(node) = self.get_mut(id) {
            node.set_tab_index_override(tab_index);
        }
        self.register_focusable(id);
    }

    pub(crate) fn set_focus_handle(&mut self, id: WidgetId, handle: Option<FocusHandle>) {
        // 停止树不得把旧身份重新绑定到外部 AppState。
        assert!(self.accepts_coordination_work());
        // 全局空 sidecar 不可能持有当前节点，空声明无需执行两次带哈希寻址。
        if handle.is_none() && self.focus_handles.is_empty() {
            return;
        }
        let unchanged = self
            .focus_handles
            .get(&id)
            .zip(handle.as_ref())
            .is_some_and(|(current, next)| current.same_handle(next));
        if unchanged {
            return;
        }
        if let Some(previous) = self.focus_handles.remove(&id) {
            previous.unbind(id);
        }
        let Some(handle) = handle else {
            return;
        };
        if let Some(app_state) = &self.app_state {
            handle.bind(id, app_state);
        }
        self.focus_handles.insert(id, handle);
    }

    /// 返回当前窗口内悬浮层栈的共享借用。
    pub fn overlay_stack(&self) -> &OverlayStack {
        &self.overlay_stack
    }

    /// 返回当前窗口内悬浮层栈的可变借用。
    pub fn overlay_stack_mut(&mut self) -> &mut OverlayStack {
        // 停止树不得重新持有悬浮层内容或用户资源。
        assert!(self.accepts_coordination_work());
        &mut self.overlay_stack
    }

    // Tab focus navigation.

    /// 按 Tab 顺序收集当前可见、未移除且允许聚焦的节点。
    pub fn collect_focusable(&self) -> Vec<WidgetId> {
        let mut result = self
            .managers
            .focus
            .focusable_order()
            .into_iter()
            .filter(|&id| self.is_tab_focus_candidate(id))
            .collect::<Vec<_>>();

        for &id in self.traverse().iter() {
            if !result.contains(&id) && self.is_tab_focus_candidate(id) {
                result.push(id);
            }
        }

        result
    }

    pub(crate) fn is_effectively_visible(&self, id: WidgetId) -> bool {
        let mut current = Some(id);
        while let Some(current_id) = current {
            let Some(node) = self.get(current_id) else {
                return false;
            };
            if !node.visible() {
                return false;
            }
            current = node.parent();
        }
        true
    }

    pub(crate) fn is_tab_focus_candidate(&self, id: WidgetId) -> bool {
        self.is_effectively_visible(id)
            && !self.is_pending_removal_subtree(id)
            && self.get(id).is_some_and(|node| node.is_focusable())
    }

    pub(crate) fn focus_target_available(&self, id: WidgetId) -> bool {
        self.is_effectively_visible(id) && self.focus_target_interactive(id)
    }

    /// 在调用方已经确认有效可见性后，验证焦点目标剩余的交互门控。
    pub(crate) fn focus_target_interactive(&self, id: WidgetId) -> bool {
        !self.is_pending_removal_subtree(id)
            && self
                .get(id)
                .is_some_and(|node| node.accepts_events() && node.is_interaction_enabled())
    }

    pub(crate) fn is_descendant_of(&self, id: WidgetId, ancestor: WidgetId) -> bool {
        let mut current = Some(id);
        while let Some(current_id) = current {
            if current_id == ancestor {
                return true;
            }
            current = self.get(current_id).and_then(|node| node.parent());
        }
        false
    }

    pub(crate) fn collect_focusable_within(&self, root: WidgetId) -> Vec<WidgetId> {
        self.collect_focusable()
            .into_iter()
            .filter(|&id| self.is_descendant_of(id, root))
            .collect()
    }

    pub(crate) fn next_focus_from_order(
        &self,
        focusable: &[WidgetId],
        current: Option<WidgetId>,
        forward: bool,
    ) -> Option<WidgetId> {
        next_focus_in_order(focusable, current, forward)
    }

    /// 返回当前焦点在 Tab 顺序中的相邻目标，但不直接改变焦点。
    pub fn focus_next(&self, forward: bool) -> Option<WidgetId> {
        let focusable = self.collect_focusable();
        self.next_focus_from_order(&focusable, self.managers.focus.focused_widget(), forward)
    }

    pub(crate) fn focus_next_in_scope(&self, root: WidgetId, forward: bool) -> Option<WidgetId> {
        let focusable = self.collect_focusable_within(root);
        self.next_focus_from_order(&focusable, self.managers.focus.focused_widget(), forward)
    }

    pub(crate) fn remember_focus_before_trap(&mut self, owner: WidgetId) {
        if self
            .focus_trap_restore
            .iter()
            .any(|&(restore_owner, _)| restore_owner == owner)
        {
            return;
        }
        let current = self.managers.focus.focused_widget();
        let restore = current.filter(|&id| !self.is_descendant_of(id, owner));
        self.focus_trap_restore.push((owner, restore));
    }

    pub(crate) fn take_focus_trap_restore(&mut self, owner: WidgetId) -> Option<WidgetId> {
        let index = self
            .focus_trap_restore
            .iter()
            .position(|&(restore_owner, _)| restore_owner == owner)?;
        self.focus_trap_restore.remove(index).1
    }

    pub(crate) fn restore_focus_after_trap_owner(&mut self, owner: WidgetId) {
        let restore_focus = self
            .take_focus_trap_restore(owner)
            .filter(|&id| self.focus_target_available(id));
        self.set_focus(restore_focus);
    }

    pub(crate) fn register_focusable(&mut self, id: WidgetId) {
        if let Some(node) = self.get(id) {
            self.managers.focus.register_focusable(id, node.tab_index());
        }
    }
}
