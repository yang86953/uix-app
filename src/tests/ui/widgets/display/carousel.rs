use crate::tests::common::*;
use crate::ui::widgets::display::carousel::Carousel;
use crate::ui::widgets::{Button, Container, Label};
use crate::ui::LayoutChild;

fn layout_carousel(carousel: &Carousel, count: usize) -> Vec<(ComponentId, Rect)> {
    let children = (0..count)
        .map(|index| LayoutChild::new(ComponentId::new(index), Size::new(80.0, 40.0)))
        .collect::<Vec<_>>();
    carousel.layout_children(
        Rect::new(10.0, 20.0, 300.0, 200.0),
        &children,
        &WidgetTree::new(),
    )
}

#[test]
fn carousel_layout_keeps_only_the_current_slide_visible() {
    let carousel = Carousel::new();

    let placements = layout_carousel(&carousel, 3);

    assert_eq!(carousel.slide_count(), 3);
    assert_eq!(WidgetComponent::tab_index(&carousel), 1);
    assert_eq!(placements[0].1, Rect::new(10.0, 20.0, 300.0, 200.0));
    assert_eq!(placements[1].1, Rect::new(10.0, 20.0, 0.0, 0.0));
    assert_eq!(placements[2].1, Rect::new(10.0, 20.0, 0.0, 0.0));
}

#[test]
fn carousel_keyboard_navigation_wraps_and_emits_index() {
    let mut carousel = Carousel::new();
    layout_carousel(&carousel, 3);
    let right = SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    };

    assert_eq!(
        carousel.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(carousel.on_event(&right), EventResult::Handled);
    assert_eq!(carousel.current_index(), 1);
    assert_eq!(
        carousel
            .semantic_event(ComponentId::new(9), &right)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("1".to_string())
    );

    carousel.on_event(&SystemEvent::KeyDown {
        key: KeyCode::End,
        mods: KeyMod::NONE,
    });
    assert_eq!(carousel.current_index(), 2);
    carousel.on_event(&right);
    assert_eq!(carousel.current_index(), 0);
}

#[test]
fn carousel_pointer_navigation_uses_node_local_coordinates() {
    let mut carousel = Carousel::new();
    layout_carousel(&carousel, 3);

    assert_eq!(
        carousel.on_event(&SystemEvent::PointerDown {
            pos: Point::new(150.0, 185.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(carousel.current_index(), 1);
}

#[test]
fn carousel_snapshot_and_accessibility_expose_runtime_slide() {
    let mut carousel = Carousel::new();
    layout_carousel(&carousel, 3);
    carousel.on_event(&SystemEvent::FocusIn);
    carousel.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    let fields = carousel.snapshot_fields();

    assert_eq!(
        fields,
        SnapshotFields::Carousel {
            show_dots: true,
            show_arrows: true,
            fixed_width: None,
            fixed_height: None,
            current: 2,
            slide_count: 3,
        }
    );
    let accessibility = fields.accessibility();
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("Slide 3 of 3")
    );
    assert_eq!(accessibility.state.value_now, Some(3.0));
    assert_eq!(accessibility.state.value_max, Some(3.0));
}

#[test]
fn carousel_size_sets_the_preferred_viewport_and_disables_flex_growth() {
    let carousel = Carousel::new().size(140.0, 88.0);

    assert_eq!(
        carousel.measure(Constraints::loose(Size::new(500.0, 500.0))),
        Size::new(140.0, 88.0)
    );
    assert_eq!(carousel.flex_grow(), 0.0);
    assert_eq!(
        carousel.snapshot_fields(),
        SnapshotFields::Carousel {
            show_dots: true,
            show_arrows: true,
            fixed_width: Some(140.0),
            fixed_height: Some(88.0),
            current: 0,
            slide_count: 0,
        }
    );
}

#[test]
fn carousel_hides_inactive_slide_subtrees_from_focus_and_hit_testing() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Carousel::new()));
    let first = tree.add_child(root, Box::new(Container::new()));
    let first_button = tree.add_child(first, Box::new(Button::new("First")));
    let second = tree.add_child(root, Box::new(Container::new()));
    let second_button = tree.add_child(second, Box::new(Button::new("Second")));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));

    tree.layout();

    assert!(tree.get(first).unwrap().visible());
    assert!(!tree.get(second).unwrap().visible());
    assert!(tree.is_effectively_visible(first_button));
    assert!(!tree.is_effectively_visible(second_button));
    assert!(tree.collect_focusable().contains(&first_button));
    assert!(!tree.collect_focusable().contains(&second_button));

    tree.set_focus(Some(root));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.layout();

    assert!(!tree.get(first).unwrap().visible());
    assert!(tree.get(second).unwrap().visible());
    assert!(!tree.is_effectively_visible(first_button));
    assert!(tree.is_effectively_visible(second_button));
    assert!(!tree.collect_focusable().contains(&first_button));
    assert!(tree.collect_focusable().contains(&second_button));
}

#[test]
fn carousel_controls_capture_pointer_over_interactive_slides_and_take_focus() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Carousel::new()));
    let first = tree.add_child(root, Box::new(Button::new("First")));
    let _second = tree.add_child(root, Box::new(Button::new("Second")));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    tree.layout();

    tree.set_focus(Some(first));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled,
        "a focused slide control must retain its arrow-key behavior"
    );
    assert_eq!(
        tree.get(root)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<Carousel>()
            .unwrap()
            .current_index(),
        0
    );

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(295.0, 100.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(root));
    assert_eq!(
        tree.get(root)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<Carousel>()
            .unwrap()
            .current_index(),
        1
    );
}

#[test]
fn carousel_narrow_controls_keep_both_arrows_and_dots_reachable() {
    let mut carousel = Carousel::new();
    let children = (0..20)
        .map(|index| LayoutChild::new(ComponentId::new(index), Size::new(10.0, 10.0)))
        .collect::<Vec<_>>();
    carousel.layout_children(
        Rect::new(0.0, 0.0, 24.0, 40.0),
        &children,
        &WidgetTree::new(),
    );

    assert_eq!(
        carousel.on_event(&SystemEvent::PointerDown {
            pos: Point::new(1.0, 10.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(carousel.current_index(), 19);
    assert_eq!(
        carousel.on_event(&SystemEvent::PointerDown {
            pos: Point::new(23.0, 10.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(carousel.current_index(), 0);
    assert_eq!(
        carousel.on_event(&SystemEvent::PointerDown {
            pos: Point::new(12.5, 32.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(carousel.current_index(), 10);
}

#[test]
fn carousel_clamps_current_when_children_are_removed() {
    let mut carousel = Carousel::new();
    layout_carousel(&carousel, 3);
    carousel.on_event(&SystemEvent::KeyDown {
        key: KeyCode::End,
        mods: KeyMod::NONE,
    });

    let placements = layout_carousel(&carousel, 1);

    assert_eq!(carousel.current_index(), 0);
    assert_eq!(carousel.slide_count(), 1);
    assert_eq!(WidgetComponent::tab_index(&carousel), 0);
    assert_eq!(placements[0].1, Rect::new(10.0, 20.0, 300.0, 200.0));
}

#[test]
fn carousel_change_requests_narrow_layout_and_swaps_real_child_frames() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Carousel::new()));
    let first = tree.add_child(root, Box::new(Label::new("First")));
    let second = tree.add_child(root, Box::new(Label::new("Second")));
    tree.get_mut(root)
        .expect("carousel root")
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    tree.layout();
    assert!(
        tree.get(root).unwrap().is_focusable(),
        "child-dependent tab_index must refresh after Carousel layout"
    );
    assert!(tree.collect_focusable().contains(&root));
    tree.set_focus(Some(root));
    tree.invalidation().lock().unwrap().clear();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(tree
        .invalidation()
        .lock()
        .unwrap()
        .layout_roots()
        .contains(&root));

    tree.layout();
    assert_eq!(
        tree.get(first).unwrap().frame(),
        Rect::new(0.0, 0.0, 0.0, 0.0)
    );
    assert_eq!(
        tree.get(second).unwrap().frame(),
        Rect::new(0.0, 0.0, 300.0, 200.0)
    );
}
