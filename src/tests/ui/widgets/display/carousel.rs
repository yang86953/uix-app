use crate::tests::common::*;
use crate::ui::widgets::display::carousel::Carousel;
use crate::ui::widgets::Label;
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
