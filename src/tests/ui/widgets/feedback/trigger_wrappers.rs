use crate::tests::common::*;
use crate::ui::widgets::{Button, Popconfirm, Popover, Tooltip};

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
