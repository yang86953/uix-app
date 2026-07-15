use crate::tests::common::*;
use crate::ui::widgets::navigation::tabs::*;

fn key_event(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    }
}

#[test]
fn tabs_are_focusable_and_keyboard_navigation_wraps() {
    let mut tabs = Tabs::new()
        .tab("Overview", "overview")
        .tab("Details", "details")
        .active(1);

    assert_eq!(WidgetComponent::tab_index(&tabs), 1);
    assert_eq!(tabs.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    tabs.on_event(&key_event(KeyCode::Right));
    assert_eq!(tabs.active_key(), Some("overview"));
    tabs.on_event(&key_event(KeyCode::Left));
    assert_eq!(tabs.active_key(), Some("details"));
    tabs.on_event(&key_event(KeyCode::Home));
    assert_eq!(tabs.active_index(), 0);
    tabs.on_event(&key_event(KeyCode::End));
    assert_eq!(tabs.active_index(), 1);
}

#[test]
fn tabs_keyboard_change_emits_active_key_and_snapshot_exposes_state() {
    let mut tabs = Tabs::new()
        .tab("Overview", "overview")
        .tab("Details", "details");
    let event = key_event(KeyCode::Right);
    tabs.on_event(&event);

    let semantic = tabs
        .semantic_event(ComponentId::new(4), &event)
        .expect("tab change should emit semantic event");
    assert_eq!(semantic.text_payload(), Some("details"));
    assert!(matches!(
        tabs.snapshot_fields(),
        SnapshotFields::Tabs {
            active_index: 1,
            tabs,
            ..
        } if tabs[1].key == "details"
    ));
    assert_eq!(
        tabs.snapshot_fields()
            .accessibility()
            .state
            .value_text
            .as_deref(),
        Some("Details")
    );
}

#[test]
fn tabs_reconcile_clamps_preserved_selection_when_items_shrink() {
    let mut tabs = Tabs::new()
        .tab("One", "one")
        .tab("Two", "two")
        .tab("Three", "three")
        .active(2);

    tabs.sync_from(Tabs::new().tab("Only", "only"));

    assert_eq!(tabs.active_index(), 0);
    assert_eq!(tabs.active_key(), Some("only"));
}
