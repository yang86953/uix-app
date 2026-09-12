use crate::ui::Widget;

pub(crate) fn builtin_widget_config_changed_without_snapshot(current: &dyn Widget, next: &dyn Widget) -> Option<bool> {
    current.declaration_config_changed(next)
}
pub(crate) fn builtin_widget_layout_changed_without_snapshot(current: &dyn Widget, next: &dyn Widget) -> Option<bool> {
    current.declaration_layout_changed(next)
}

pub(crate) fn builtin_widget_config_changed(current: &crate::ui::WidgetSnapshotFields, next: &crate::ui::WidgetSnapshotFields) -> Option<bool> {
    Some(current.config_changed(next))
}
pub(crate) fn builtin_widget_layout_changed(current: &crate::ui::WidgetSnapshotFields, next: &crate::ui::WidgetSnapshotFields) -> Option<bool> {
    Some(current.layout_changed(next))
}

pub(crate) fn builtin_widget_runtime_changed(current: &dyn Widget, next: &dyn Widget) -> bool {
    current.declaration_runtime_changed(next)
}

pub(crate) fn patch_builtin_widget(
    current: &mut dyn Widget,
    next: Box<dyn Widget>,
) -> std::result::Result<bool, Box<dyn Widget>> {
    current.reconcile_from(next)
}
