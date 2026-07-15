use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::navigation::anchor::*;

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
fn anchor_is_focusable_and_keyboard_navigation_emits_href() {
    let mut anchor = Anchor::new(vec![
        AnchorItem::new("Basic", "#basic"),
        AnchorItem::new("Advanced", "#advanced"),
    ]);
    let event = SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&anchor), 1);
    assert_eq!(anchor.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(anchor.on_event(&event), EventResult::Handled);
    assert_eq!(anchor.active_href(), "#advanced");
    assert_eq!(
        anchor
            .semantic_event(ComponentId::new(8), &event)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("#advanced".to_string())
    );
    assert_eq!(
        anchor.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}

#[test]
fn anchor_enter_reactivates_current_target() {
    let mut anchor = Anchor::new(vec![AnchorItem::new("Basic", "#basic")]);
    let event = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };

    assert_eq!(anchor.on_event(&event), EventResult::Handled);
    assert_eq!(
        anchor
            .semantic_event(ComponentId::new(9), &event)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("#basic".to_string())
    );
}

#[test]
fn anchor_snapshot_and_accessibility_expose_active_item() {
    let mut anchor = Anchor::new(vec![
        AnchorItem::new("Basic", "#basic"),
        AnchorItem::new("Advanced", "#advanced"),
    ]);
    anchor.update_active(100.0);
    let fields = anchor.snapshot_fields();

    assert!(matches!(
        fields,
        SnapshotFields::Anchor {
            active_index: 1,
            ..
        }
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Advanced"));
    assert_eq!(accessibility.state.value_now, Some(2.0));
}

#[test]
fn empty_anchor_is_not_focusable_and_negative_pointer_is_ignored() {
    let mut empty = Anchor::new(Vec::new());
    assert_eq!(WidgetComponent::tab_index(&empty), 0);
    assert_eq!(
        empty.on_event(&SystemEvent::PointerDown {
            pos: Point::new(0.0, -1.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}
