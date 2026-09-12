//! Generic tree coordination through component-owned semantic ports.
use crate::ui::widget_runtime::widget::{WidgetCore, WidgetId, WidgetTree};

pub(crate) fn nearest_viewport_overflow_axes(tree: &WidgetTree, id: WidgetId) -> Option<(bool, bool)> {
    let mut current = id;
    while let Some(parent_id) = tree.get(current).and_then(|node| node.parent()) {
        let parent = tree.get(parent_id)?;
        if parent.children_clip(parent.frame()).is_some() {
            return Some(parent.widget().viewport_overflow_axes());
        }
        current = parent_id;
    }
    None
}
pub(crate) fn phase2_explicit_size_locks(tree: &WidgetTree, id: WidgetId) -> (bool, bool) {
    tree.get(id).map(|node| node.widget().layout_size_locks()).unwrap_or((false, false))
}
pub(crate) fn modal_was_present(tree: &WidgetTree, id: WidgetId) -> bool {
    tree.get(id).is_some_and(|node| node.widget().overlay_is_present())
}
pub(crate) fn modal_closed_for_destruction(tree: &WidgetTree, id: WidgetId) -> bool {
    tree.get(id).is_some_and(|node| node.widget().overlay_destroy_on_close())
}
pub(crate) fn apply_modal_context_requests(tree: &mut WidgetTree, path: &[WidgetId]) {
    let mut changed = false;
    for id in path {
        if let Some(node) = tree.get_mut(*id) {
            if node.widget_mut().consume_context_close() {
                node.set_active(true);
                changed = true;
            }
        }
    }
    if changed {
        tree.mark_full_frame_dirty();
        tree.rebuild_widget_overlays();
    }
}
pub(crate) fn dismiss_overlay_owner_from_outside(tree: &mut WidgetTree, owner: WidgetId) {
    if let Some(node) = tree.get_mut(owner) { node.widget_mut().dismiss_overlay(); }
}
pub(crate) fn pointer_focus_target(tree: &WidgetTree, target: WidgetId) -> WidgetId {
    if tree.get(target).is_some_and(|node| node.widget().pointer_focus_descendant()) {
        if let Some(trigger) = tree.collect_focusable_within(target).into_iter().next() { return trigger; }
    }
    target
}
pub(crate) fn widget_input_value(widget: &dyn crate::ui::Widget) -> Option<String> {
    widget.semantic_text_value()
}
pub(crate) fn is_drag_region(tree: &WidgetTree, id: WidgetId) -> bool {
    tree.get(id).is_some_and(|node| node.widget().window_drag_region())
}
pub(crate) fn invalidate_nav_siblings(tree: &mut WidgetTree, clicked: WidgetId) {
    if tree.get(clicked).is_some_and(|node| node.widget().invalidate_action_siblings()) {
        if let Some(parent) = tree.get(clicked).and_then(|node| node.parent()) {
            tree.invalidate_paint_subtree(parent);
        }
    }
}
pub(crate) fn focus_replacement_after_child_visibility(tree: &WidgetTree, focused: WidgetId, changes: &[(WidgetId, bool)]) -> Option<WidgetId> {
    let hidden = changes.iter().find_map(|(child, visible)|
        (!*visible && tree.is_descendant_of(focused, *child)).then_some(*child))?;
    let parent_id = tree.get(hidden)?.parent()?;
    let parent = tree.get(parent_id)?;
    let index = parent.widget().preferred_focus_child()?;
    let active = parent.children().get(index).copied();
    if let Some(target) = active.and_then(|panel| tree.collect_focusable_within(panel).into_iter().next()) {
        return Some(target);
    }
    (parent.is_focusable() && tree.focus_target_available(parent_id)).then_some(parent_id)
}
