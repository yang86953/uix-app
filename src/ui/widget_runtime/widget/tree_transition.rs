use super::tree_core::WidgetTree;
use super::*;

impl WidgetTree {
    pub(crate) fn start_leave_transition(&mut self, id: WidgetId) -> bool {
        if self.get(id).is_some_and(BoxedWidget::pending_removal) {
            return self
                .get(id)
                .is_some_and(BoxedWidget::view_transition_active);
        }

        let old_bounds = self.visual_subtree_bounds(id);
        self.cancel_subtree_interaction(id);
        let started = self
            .get_mut(id)
            .is_some_and(BoxedWidget::start_leave_transition);
        if !started {
            return false;
        }

        self.tree_version = self.tree_version.wrapping_add(1);
        let new_bounds = self.visual_subtree_bounds(id);
        for rect in [old_bounds, new_bounds].into_iter().flatten() {
            if rect.w > 0.0 && rect.h > 0.0 {
                self.push_paint_invalidation(id, Some(rect));
            }
        }
        self.rebuild_widget_overlays();
        true
    }

    pub(crate) fn cancel_pending_removal(&mut self, id: WidgetId) -> bool {
        let old_bounds = self.visual_subtree_bounds(id);
        let cancelled = self
            .get_mut(id)
            .is_some_and(BoxedWidget::cancel_pending_removal);
        if !cancelled {
            return false;
        }

        self.tree_version = self.tree_version.wrapping_add(1);
        let new_bounds = self.visual_subtree_bounds(id);
        for rect in [old_bounds, new_bounds].into_iter().flatten() {
            if rect.w > 0.0 && rect.h > 0.0 {
                self.push_paint_invalidation(id, Some(rect));
            }
        }
        self.rebuild_widget_overlays();
        true
    }

    pub fn is_pending_removal_subtree(&self, id: WidgetId) -> bool {
        let mut current = Some(id);
        while let Some(current_id) = current {
            let Some(node) = self.get(current_id) else {
                return false;
            };
            if node.pending_removal() {
                return true;
            }
            current = node.parent();
        }
        false
    }
}
