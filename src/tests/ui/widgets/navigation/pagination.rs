use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::navigation::pagination::*;

#[test]
fn pagination_click_emits_change_semantic_event() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Pagination::new(85, 10)));
    tree.get_mut(id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 260.0, 40.0));

    let page = Rc::new(RefCell::new(String::new()));
    let page_for_handler = page.clone();
    tree.handler_table()
        .on(id, SemanticKind::Change, move |event| {
            if let Some(value) = event.text_payload() {
                *page_for_handler.borrow_mut() = value.to_string();
            }
        });

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(70.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(&*page.borrow(), "2");
}

#[test]
fn pagination_normalizes_zero_page_size_and_extreme_current() {
    let mut pagination = Pagination::new(100, 0).page_size(0).current(usize::MAX);

    assert_eq!(pagination.total_pages(), 100);
    assert_eq!(pagination.get_current(), 100);
    assert_eq!(
        pagination.visible_range(100, pagination.get_current()),
        vec![1, 0, 99, 100]
    );

    assert_eq!(
        pagination.on_event(&SystemEvent::PointerDown {
            pos: Point::new(170.0, 0.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(pagination.get_current(), 100);

    assert_eq!(Pagination::new(100, 10).current(0).get_current(), 1);
}

#[test]
fn pagination_is_focusable_and_keyboard_changes_page() {
    let mut pagination = Pagination::new(100, 10).current(5);

    assert_eq!(WidgetComponent::tab_index(&pagination), 1);
    assert_eq!(
        pagination.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    pagination.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(pagination.get_current(), 6);
    pagination.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Home,
        mods: KeyMod::NONE,
    });
    assert_eq!(pagination.get_current(), 1);
    pagination.on_event(&SystemEvent::KeyDown {
        key: KeyCode::End,
        mods: KeyMod::NONE,
    });
    assert_eq!(pagination.get_current(), 10);
}

#[test]
fn pagination_page_size_changer_is_interactive_and_emits_change() {
    let mut pagination = Pagination::new(100, 10)
        .show_total(false)
        .show_size_changer(true)
        .page_size_options(vec![10, 25, 50]);
    let event = SystemEvent::PointerDown {
        pos: Point::new(210.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(pagination.on_event(&event), EventResult::Handled);
    assert_eq!(pagination.get_page_size(), 25);
    let semantic = pagination
        .semantic_event(ComponentId::new(9), &event)
        .expect("page-size change should emit event");
    assert_eq!(semantic.text_payload(), Some("page_size=25"));

    pagination.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Up,
        mods: KeyMod::NONE,
    });
    assert_eq!(pagination.get_page_size(), 10);
}

#[test]
fn pagination_snapshot_and_accessibility_expose_runtime_page() {
    let pagination = Pagination::new(95, 10).current(3);
    let fields = pagination.snapshot_fields();

    assert!(matches!(
        fields,
        SnapshotFields::Pagination {
            current: 3,
            page_size: 10,
            ..
        }
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.state.value_now, Some(3.0));
    assert_eq!(accessibility.state.value_max, Some(10.0));
}
