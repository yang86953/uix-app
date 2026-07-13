use crate::tests::common::*;
use crate::ui::foundation::focus_trap::*;

#[test]
fn next_focus_in_order_wraps_forward_and_backward() {
    let ids = [
        ComponentId::new(1),
        ComponentId::new(3),
        ComponentId::new(5),
    ];

    assert_eq!(
        next_focus_in_order(&ids, Some(ComponentId::new(1)), true),
        Some(ComponentId::new(3))
    );
    assert_eq!(
        next_focus_in_order(&ids, Some(ComponentId::new(5)), true),
        Some(ComponentId::new(1))
    );
    assert_eq!(
        next_focus_in_order(&ids, Some(ComponentId::new(1)), false),
        Some(ComponentId::new(5))
    );
    assert_eq!(
        next_focus_in_order(&ids, Some(ComponentId::new(9)), true),
        Some(ComponentId::new(1))
    );
    assert_eq!(
        next_focus_in_order(&ids, None, false),
        Some(ComponentId::new(5))
    );
    assert_eq!(
        next_focus_in_order(&ids, Some(ComponentId::new(9)), false),
        Some(ComponentId::new(5))
    );
}

#[test]
fn focus_trap_component_does_not_intercept_tab_without_tree_scope() {
    let mut trap = FocusTrap::new();

    assert_eq!(
        trap.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}

#[test]
fn focus_trap_next_focus_uses_sorted_focusable_ids() {
    let mut ids = HashSet::new();
    ids.insert(ComponentId::new(8));
    ids.insert(ComponentId::new(2));
    ids.insert(ComponentId::new(5));
    let mut trap = FocusTrap::new();
    trap.update_focusable(ids);

    assert_eq!(
        trap.next_focus(Some(ComponentId::new(2)), true),
        Some(ComponentId::new(5))
    );
    assert_eq!(
        trap.active(false)
            .next_focus(Some(ComponentId::new(2)), true),
        None
    );
}
