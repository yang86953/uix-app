use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::navigation::menu::*;

#[test]
fn menu_selection_emits_change_semantic_event() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(
        Menu::new()
            .add_item(MenuItem {
                key: "home".into(),
                label: "首页".into(),
                icon: String::new(),
                disabled: false,
            })
            .add_item(MenuItem {
                key: "docs".into(),
                label: "文档".into(),
                icon: String::new(),
                disabled: false,
            })
            .active_key("home"),
    ));
    tree.get_mut(id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 32.0));

    let selected = Rc::new(RefCell::new(String::new()));
    let selected_for_handler = selected.clone();
    tree.handler_table()
        .on(id, SemanticKind::Change, move |event| {
            if let Some(value) = event.text_payload() {
                *selected_for_handler.borrow_mut() = value.to_string();
            }
        });

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(90.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(&*selected.borrow(), "docs");
}

#[test]
fn menu_is_focusable_and_keyboard_navigation_wraps_while_skipping_disabled_items() {
    let mut menu = Menu::new()
        .add_item(MenuItem {
            key: "home".into(),
            label: "Home".into(),
            icon: String::new(),
            disabled: false,
        })
        .add_item(MenuItem {
            key: "disabled".into(),
            label: "Disabled".into(),
            icon: String::new(),
            disabled: true,
        })
        .add_item(MenuItem {
            key: "docs".into(),
            label: "Docs".into(),
            icon: String::new(),
            disabled: false,
        });

    assert_eq!(WidgetComponent::tab_index(&menu), 1);
    assert_eq!(
        menu.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(menu.get_active_key(), "home");

    menu.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(menu.get_active_key(), "docs");

    menu.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(menu.get_active_key(), "home");

    menu.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(menu.get_active_key(), "docs");
}

#[test]
fn vertical_menu_only_handles_vertical_navigation_keys() {
    let mut menu = Menu::new()
        .add_item(MenuItem {
            key: "home".into(),
            label: "Home".into(),
            icon: String::new(),
            disabled: false,
        })
        .mode(MenuMode::Vertical);

    assert_eq!(
        menu.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        menu.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(menu.get_active_key(), "home");
}
