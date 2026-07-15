use crate::tests::common::*;
use crate::ui::widgets::{SelectableItem, SelectableList};
use crate::ui::AccessibilityRole;

fn list_with_items(count: usize) -> SelectableList {
    SelectableList::new()
        .items(
            (0..count)
                .map(|index| SelectableItem::new(format!("item-{index}"), format!("Item {index}")))
                .collect(),
        )
        .footer(format!("{count} items"))
}

#[test]
fn selectable_list_keyboard_selection_scrolls_and_emits_item_id() {
    let mut list = list_with_items(100);
    list.last_frame.set(Some(Rect::new(0.0, 0.0, 220.0, 120.0)));
    let end = SystemEvent::KeyDown {
        key: KeyCode::End,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&list), 1);
    assert_eq!(list.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(list.on_event(&end), EventResult::Handled);
    assert_eq!(list.selected_id(), Some("item-99"));
    assert!(list.body_scroll.scroll_offset() > 0.0);
    assert!(EventHandler::scroll_delta_for_dirty(&list).is_some());
    assert_eq!(
        list.semantic_event(ComponentId::new(7), &end)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("item-99".to_string())
    );
}

#[test]
fn selectable_list_snapshot_and_accessibility_report_runtime_selection() {
    let mut list = list_with_items(3).active(1);
    list.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });

    assert_eq!(list.selected_text(), Some("Item 2"));
    let fields = list.snapshot_fields();
    let SnapshotFields::SelectableList { active_index, .. } = fields.clone() else {
        panic!("expected selectable list snapshot");
    };
    assert_eq!(active_index, 2);
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::List);
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Item 2"));
    assert_eq!(accessibility.state.value_now, Some(3.0));
}

#[test]
fn selectable_list_header_emits_submit_and_footer_does_not_hit_rows() {
    let mut list = list_with_items(20).header_button("Add");
    list.last_frame.set(Some(Rect::new(0.0, 0.0, 220.0, 140.0)));
    let header_click = SystemEvent::PointerDown {
        pos: Point::new(20.0, 20.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(list.on_event(&header_click), EventResult::Handled);
    let event = list
        .semantic_event(ComponentId::new(5), &header_click)
        .expect("header action should emit a semantic event");
    assert_eq!(event.kind, SemanticKind::Submit);
    assert_eq!(event.text_payload(), Some("Add"));

    assert_eq!(
        list.on_event(&SystemEvent::PointerDown {
            pos: Point::new(20.0, 130.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}

#[test]
fn empty_selectable_list_without_header_is_not_focusable() {
    let list = SelectableList::new();

    assert_eq!(WidgetComponent::tab_index(&list), 0);
    assert_eq!(
        WidgetComponent::tab_index(&SelectableList::new().header_button("Add")),
        1
    );
}
