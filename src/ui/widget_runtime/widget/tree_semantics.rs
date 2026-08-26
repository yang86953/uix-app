use super::tree_core::WidgetTree;
use super::*;
use crate::ui::event::SemanticEvent;

impl WidgetTree {
    fn fill_semantic_path_to_root(&self, target: WidgetId, path: &mut Vec<WidgetId>) {
        path.clear();
        let mut current = Some(target);
        while let Some(id) = current {
            path.push(id);
            current = self.get(id).and_then(|node| node.parent());
        }
    }

    /// 沿目标到根的路径分发可变语义事件，并应用处理器请求。
    pub fn dispatch_semantic_event(&mut self, event: &mut SemanticEvent) -> EventResult {
        // 停止树不得通过语义分发触发 handler 或后续布局请求。
        if !self.accepts_external_work() {
            // 对调用方明确报告语义事件未处理。
            return EventResult::NotHandled;
        }
        if self.is_pending_removal_subtree(event.target) {
            return EventResult::NotHandled;
        }
        // 暂时取走树级工作区，使用户回调重入时内层分发使用独立空槽。
        let mut path = std::mem::take(&mut self.semantic_path_scratch);
        self.fill_semantic_path_to_root(event.target, &mut path);
        let result = self.handler_table.dispatch_path(&path, event);
        self.apply_modal_context_requests(&path);
        self.apply_semantic_layout_requests(&path);
        // 保留路径容量，消除稳态短路径的重复申请。
        self.semantic_path_scratch = path;
        result
    }

    /// 按值接收并分发语义事件。
    pub fn dispatch_semantic(&mut self, mut event: SemanticEvent) -> EventResult {
        self.dispatch_semantic_event(&mut event)
    }

    fn apply_modal_context_requests(&mut self, path: &[WidgetId]) {
        crate::ui::tree_widget_hooks::apply_modal_context_requests(self, path);
    }

    fn apply_semantic_layout_requests(&mut self, path: &[WidgetId]) {
        for &id in path {
            let requested = self
                .get_mut(id)
                .is_some_and(|node| node.take_layout_request());
            if requested {
                self.push_layout_invalidation(id);
                self.propagate_layout_invalidation(id);
            }
        }
    }
}
