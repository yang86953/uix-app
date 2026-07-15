use crate::tests::common::*;
use crate::ui::widgets::navigation::dropdown::*;

fn key_event(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    }
}

#[test]
fn dropdown_is_focusable_and_keyboard_selects_items() {
    let mut dropdown = Dropdown::new("Actions").items(vec!["Edit", "Delete", "Export"]);

    assert_eq!(WidgetComponent::tab_index(&dropdown), 1);
    assert_eq!(
        dropdown.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    dropdown.on_event(&key_event(KeyCode::Down));
    assert!(dropdown.is_open());
    dropdown.on_event(&key_event(KeyCode::Down));
    dropdown.on_event(&key_event(KeyCode::Enter));

    assert_eq!(dropdown.selected_index(), Some(1));
    assert_eq!(dropdown.current_value(), Some("Delete"));
    assert!(!dropdown.is_open());
}

#[test]
fn dropdown_pointer_and_keyboard_selection_emit_change_values() {
    let mut dropdown = Dropdown::new("Actions").items(vec!["Edit", "Delete"]);
    dropdown.open();
    let event = SystemEvent::PointerDown {
        pos: Point::new(20.0, 47.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(dropdown.on_event(&event), EventResult::Handled);
    let semantic = dropdown
        .semantic_event(ComponentId::new(3), &event)
        .expect("selection should emit change");
    assert_eq!(semantic.text_payload(), Some("Edit"));
}

#[test]
fn dropdown_snapshot_and_accessibility_expose_runtime_state() {
    let mut dropdown = Dropdown::new("Actions").items(vec!["Edit", "Delete"]);
    dropdown.open();
    dropdown.on_event(&key_event(KeyCode::End));

    let fields = dropdown.snapshot_fields();
    assert!(matches!(
        fields,
        SnapshotFields::Dropdown {
            open: true,
            selected_index: None,
            highlighted_index: Some(1),
            ..
        }
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.name.as_deref(), Some("Actions"));
    assert_eq!(accessibility.state.expanded, Some(true));
}

#[test]
fn dropdown_escape_closes_without_selection() {
    let mut dropdown = Dropdown::new("Actions").items(vec!["Edit"]);
    dropdown.open();

    assert_eq!(
        dropdown.on_event(&key_event(KeyCode::Escape)),
        EventResult::Handled
    );
    assert!(!dropdown.is_open());
    assert_eq!(dropdown.current_value(), None);
}
