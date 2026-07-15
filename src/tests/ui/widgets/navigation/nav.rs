use crate::tests::common::*;
use crate::ui::widgets::navigation::nav::*;

#[test]
fn nav_item_is_focusable_and_keyboard_activation_emits_change() {
    let active = Rc::new(Cell::new(0));
    let mut item = NavItem::new("Settings", 1, active.clone());
    let event = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&item), 1);
    assert_eq!(item.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(item.on_event(&event), EventResult::Handled);
    assert_eq!(active.get(), 1);
    assert!(item.is_active());

    let semantic = item
        .semantic_event(ComponentId::new(5), &event)
        .expect("activation should emit change");
    assert_eq!(semantic.text_payload(), Some("1"));
}

#[test]
fn active_nav_item_does_not_emit_duplicate_change() {
    let active = Rc::new(Cell::new(2));
    let mut item = NavItem::new("Reports", 2, active);
    let event = SystemEvent::KeyDown {
        key: KeyCode::Space,
        mods: KeyMod::NONE,
    };

    assert_eq!(item.on_event(&event), EventResult::Handled);
    assert!(item.semantic_event(ComponentId::new(6), &event).is_none());
}

#[test]
fn nav_item_snapshot_and_accessibility_expose_selection() {
    let item = NavItem::new("Home", 0, Rc::new(Cell::new(0)));
    let fields = item.snapshot_fields();

    assert!(matches!(
        fields,
        SnapshotFields::NavItem {
            active: true,
            index: 0,
            ..
        }
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.name.as_deref(), Some("Home"));
    assert_eq!(accessibility.state.selected, Some(true));
}
