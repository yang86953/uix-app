use crate::prelude::Button;
use crate::tests::common::*;
use crate::ui::foundation::focus_trap::*;
use crate::ui::LayoutChild;

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

#[test]
fn focus_trap_lays_out_its_single_content_subtree_in_the_trap_frame() {
    let trap = FocusTrap::new();
    let child = ComponentId::new(11);
    let frame = Rect::new(12.0, 18.0, 320.0, 52.0);

    assert_eq!(
        trap.layout_children(
            frame,
            &[LayoutChild::new(child, Size::new(280.0, 40.0))],
            &WidgetTree::new(),
        ),
        vec![(child, frame)]
    );
}

#[test]
fn widget_tree_tabs_and_wraps_inside_standalone_focus_trap() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(FocusTrap::new()));
    let first = tree.add_child(root, Box::new(Button::new("First")));
    let second = tree.add_child(root, Box::new(Button::new("Second")));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 40.0));
    tree.layout();
    tree.set_focus(Some(first));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(second));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(first));
}
