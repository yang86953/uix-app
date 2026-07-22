use super::tree_core::WidgetTree;
use super::*;
use crate::ui::event::SemanticEvent;

impl WidgetTree {
    fn semantic_path_to_root(&self, target: WidgetId) -> Vec<WidgetId> {
        let mut path = Vec::new();
        let mut current = Some(target);
        while let Some(id) = current {
            path.push(id);
            current = self.get(id).and_then(|node| node.parent());
        }
        path
    }

    pub fn dispatch_semantic_event(&mut self, event: &mut SemanticEvent) -> EventResult {
        if self.is_pending_removal_subtree(event.target) {
            return EventResult::NotHandled;
        }
        let path = self.semantic_path_to_root(event.target);
        let result = self.handler_table.dispatch_path(&path, event);
        self.apply_modal_context_requests(&path);
        self.apply_semantic_layout_requests(&path);
        result
    }

    pub fn dispatch_semantic(&mut self, mut event: SemanticEvent) -> EventResult {
        self.dispatch_semantic_event(&mut event)
    }

    fn apply_modal_context_requests(&mut self, path: &[WidgetId]) {
        let requested: Vec<WidgetId> = path
            .iter()
            .copied()
            .filter(|&id| {
                self.get(id)
                    .and_then(|node| {
                        node.component()
                            .as_any()
                            .downcast_ref::<crate::ui::widgets::Modal>()
                    })
                    .is_some_and(|modal| modal.take_context_close_request())
            })
            .collect();
        if requested.is_empty() {
            return;
        }

        for id in requested {
            if let Some(node) = self.get_mut(id) {
                if let Some(modal) = node
                    .component_mut()
                    .as_any_mut()
                    .downcast_mut::<crate::ui::widgets::Modal>()
                {
                    modal.close();
                    node.set_active(true);
                }
            }
        }
        self.mark_full_frame_dirty();
        self.rebuild_widget_overlays();
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
