use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::navigation::anchor::*;
use crate::ui::core::widget::WidgetCore;

#[test]
fn anchor_click_emits_change_semantic_event() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Anchor::new(vec![
        AnchorItem::new("基础", "#basic"),
        AnchorItem::new("高级", "#advanced"),
        AnchorItem::new("API", "#api"),
    ])));
    tree.get_mut(id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 160.0, 108.0));

    let href = Rc::new(RefCell::new(String::new()));
    let href_for_handler = href.clone();
    tree.handler_table()
        .on(id, SemanticKind::Change, move |event| {
            if let Some(value) = event.text_payload() {
                *href_for_handler.borrow_mut() = value.to_string();
            }
        });

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(20.0, 80.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(&*href.borrow(), "#api");
}

#[test]
fn measure_clamps_anchor_size() {
    let measured = Anchor::new(vec![
        AnchorItem::new("Basic", "#basic"),
        AnchorItem::new("Advanced", "#advanced"),
    ])
    .measure(Constraints::loose(Size::new(80.0, 36.0)));

    assert_eq!(measured, Size::new(80.0, 36.0));
}
