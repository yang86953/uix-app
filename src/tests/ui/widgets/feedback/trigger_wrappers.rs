use crate::tests::common::*;
use crate::ui::widgets::{
    Button, Container, Popconfirm, Popover, PopoverTrigger, Tooltip, TriggerMode,
};
use crate::ui::{AccessibilityRole, LayoutChild};

fn set_frame(tree: &mut WidgetTree, id: ComponentId, frame: Rect) {
    tree.get_mut(id).expect("widget").set_frame(frame);
    tree.get_mut(id).expect("widget").set_active(true);
}

#[test]
fn popover_click_trigger_owns_pointer_hit_over_child_button() {
    let mut tree = WidgetTree::new();
    let wrapper = tree.set_root(Box::new(Popover::new("details")));
    let child = tree.add_child(wrapper, Box::new(Button::new("details")));
    let frame = Rect::new(120.0, 80.0, 80.0, 28.0);
    set_frame(&mut tree, wrapper, frame);
    set_frame(&mut tree, child, frame);

    let pointer = Point::new(140.0, 92.0);
    assert_eq!(tree.hit_test(pointer), Some(wrapper));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: pointer,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(tree
        .get(wrapper)
        .and_then(|node| node.component().as_any().downcast_ref::<Popover>())
        .is_some_and(Popover::is_visible));
}

#[test]
fn tooltip_hover_trigger_owns_pointer_hit_over_child_button() {
    let mut tree = WidgetTree::new();
    let wrapper = tree.set_root(Box::new(Tooltip::new("help")));
    let child = tree.add_child(wrapper, Box::new(Button::new("help")));
    let frame = Rect::new(200.0, 140.0, 80.0, 28.0);
    set_frame(&mut tree, wrapper, frame);
    set_frame(&mut tree, child, frame);

    let pointer = Point::new(220.0, 152.0);
    assert_eq!(tree.hit_test(pointer), Some(wrapper));
    let _ = tree.dispatch_event(&SystemEvent::PointerMove {
        pos: pointer,
        mods: KeyMod::NONE,
    });
    assert!(tree
        .get(wrapper)
        .and_then(|node| node.component().as_any().downcast_ref::<Tooltip>())
        .is_some_and(Tooltip::is_visible));
}

#[test]
fn tooltip_lays_out_its_trigger_child_in_the_wrapper_frame() {
    let tooltip = Tooltip::new("help");
    let child = ComponentId::new(7);
    let frame = Rect::new(24.0, 32.0, 96.0, 36.0);

    assert_eq!(
        tooltip.measure(Constraints::loose(Size::new(200.0, 100.0))),
        Size::new(80.0, 28.0)
    );

    assert_eq!(
        tooltip.layout_children(
            frame,
            &[LayoutChild::new(child, Size::new(80.0, 28.0))],
            &WidgetTree::new(),
        ),
        vec![(child, frame)]
    );
}

#[test]
fn popconfirm_trigger_owns_pointer_hit_and_confirm_emits_submit() {
    let mut tree = WidgetTree::new();
    let wrapper = tree.set_root(Box::new(Popconfirm::new()));
    let child = tree.add_child(wrapper, Box::new(Button::new("delete")));
    let frame = Rect::new(300.0, 220.0, 80.0, 28.0);
    set_frame(&mut tree, wrapper, frame);
    set_frame(&mut tree, child, frame);

    let pointer = Point::new(320.0, 232.0);
    assert_eq!(tree.hit_test(pointer), Some(wrapper));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: pointer,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(tree
        .get(wrapper)
        .and_then(|node| node.component().as_any().downcast_ref::<Popconfirm>())
        .is_some_and(Popconfirm::is_visible));

    let confirm = SystemEvent::PointerDown {
        pos: Point::new(20.0, -40.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    let popconfirm = tree
        .get_mut(wrapper)
        .and_then(|node| {
            node.component_mut()
                .as_any_mut()
                .downcast_mut::<Popconfirm>()
        })
        .expect("popconfirm");
    assert_eq!(popconfirm.on_event(&confirm), EventResult::Handled);
    let semantic = popconfirm
        .semantic_event(wrapper, &confirm)
        .expect("confirm submit");
    assert_eq!(semantic.kind, SemanticKind::Submit);
    assert_eq!(semantic.text_payload(), Some("confirm"));
}

#[test]
fn popconfirm_keyboard_selects_cancel_or_emits_confirm_submit() {
    let mut popconfirm = Popconfirm::new()
        .title("Delete item?")
        .confirm_text("Delete")
        .cancel_text("Keep");
    let id = ComponentId::new(7);

    assert_eq!(WidgetComponent::tab_index(&popconfirm), 1);
    assert_eq!(
        popconfirm.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(
        popconfirm.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(popconfirm.focused_action(), Some(0));

    let accessibility = popconfirm.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.name.as_deref(), Some("Delete item?"));
    assert_eq!(accessibility.state.expanded, Some(true));
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Delete"));

    assert_eq!(
        popconfirm.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(popconfirm.focused_action(), Some(1));
    assert_eq!(
        popconfirm.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!popconfirm.is_visible());
    assert!(popconfirm
        .semantic_event(
            id,
            &SystemEvent::KeyDown {
                key: KeyCode::Space,
                mods: KeyMod::NONE,
            }
        )
        .is_none());

    popconfirm.open();
    assert_eq!(
        popconfirm.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let semantic = popconfirm
        .semantic_event(
            id,
            &SystemEvent::KeyDown {
                key: KeyCode::Enter,
                mods: KeyMod::NONE,
            },
        )
        .expect("confirm submit");
    assert_eq!(semantic.kind, SemanticKind::Submit);
    assert_eq!(semantic.text_payload(), Some("confirm"));
}

#[test]
fn popconfirm_ignores_non_primary_pointer_and_closes_after_focus_leaves() {
    let mut popconfirm = Popconfirm::new();
    let right_click = SystemEvent::PointerDown {
        pos: Point::new(20.0, 12.0),
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    };

    assert_eq!(popconfirm.on_event(&right_click), EventResult::NotHandled);
    assert!(!popconfirm.is_visible());

    popconfirm.open();
    assert_eq!(popconfirm.on_focus_within(false), EventResult::Handled);
    assert!(!popconfirm.is_visible());
}

#[test]
fn focus_trigger_observes_descendants_without_churn_inside_the_wrapper() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new()));
    let wrapper = tree.add_child(
        root,
        Box::new(Popover::new("details").trigger(PopoverTrigger::Focus)),
    );
    let first = tree.add_child(wrapper, Box::new(Button::new("first")));
    let second = tree.add_child(wrapper, Box::new(Button::new("second")));
    let outside = tree.add_child(root, Box::new(Button::new("outside")));

    tree.set_focus(Some(first));
    assert!(tree
        .get(wrapper)
        .and_then(|node| node.component().as_any().downcast_ref::<Popover>())
        .is_some_and(Popover::is_visible));

    tree.set_focus(Some(second));
    assert!(tree
        .get(wrapper)
        .and_then(|node| node.component().as_any().downcast_ref::<Popover>())
        .is_some_and(Popover::is_visible));

    tree.set_focus(Some(outside));
    assert!(tree
        .get(wrapper)
        .and_then(|node| node.component().as_any().downcast_ref::<Popover>())
        .is_some_and(|popover| !popover.is_visible()));
}

#[test]
fn delayed_focus_tooltip_tracks_child_focus_and_cancels_on_removal() {
    let mut tree = WidgetTree::new();
    let wrapper = tree.set_root(Box::new(
        Tooltip::new("help")
            .trigger(TriggerMode::Focus)
            .delay_ms(300),
    ));
    let child = tree.add_child(wrapper, Box::new(Button::new("help")));

    tree.set_focus(Some(child));
    let tooltip = tree
        .get(wrapper)
        .and_then(|node| node.component().as_any().downcast_ref::<Tooltip>())
        .expect("tooltip");
    assert!(!tooltip.is_visible());
    assert_eq!(
        tooltip.active_timer(),
        Some((1, Duration::from_millis(300)))
    );

    tree.remove(child);
    let tooltip = tree
        .get(wrapper)
        .and_then(|node| node.component().as_any().downcast_ref::<Tooltip>())
        .expect("tooltip");
    assert_eq!(tree.managers().focus.focused_component(), None);
    assert_eq!(tooltip.active_timer(), None);
    assert!(!tooltip.is_visible());
}
