use super::*;
use crate::core::{Constraints, Size};
use crate::native::traits::input::{KeyMod, MouseButton};
use crate::ui::core::widget::WidgetCore;
use crate::ui::traits::WidgetLayout;
use crate::ui::{SemanticKind, WidgetTree};
use std::cell::RefCell;
use std::rc::Rc;

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
fn measure_clamps_menu_size() {
    let measured = Menu::new()
        .add_item(MenuItem {
            key: "home".into(),
            label: "Home".into(),
            icon: String::new(),
            disabled: false,
        })
        .measure(Constraints::loose(Size::new(80.0, 20.0)));

    assert_eq!(measured, Size::new(80.0, 20.0));
}
