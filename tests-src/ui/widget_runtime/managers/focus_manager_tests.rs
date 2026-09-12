//! `ui/widget_runtime/managers/focus_manager.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

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
