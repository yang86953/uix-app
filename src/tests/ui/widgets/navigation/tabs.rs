use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
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
    assert_eq!(tabs.current_key(), Some("overview"));
    tabs.on_event(&key_event(KeyCode::Left));
    assert_eq!(tabs.current_key(), Some("details"));
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
    assert_eq!(tabs.current_key(), Some("only"));
}

#[test]
fn string_active_key_binding_reads_and_writes_state() {
    let active = State::new("details".to_string());
    let mut tabs = Tabs::new()
        .tab("Overview", "overview")
        .tab("Details", "details")
        .active_key(&active);

    assert_eq!(tabs.current_key(), Some("details"));
    tabs.on_event(&key_event(KeyCode::Right));
    assert_eq!(tabs.current_key(), Some("overview"));
    assert_eq!(active.get(), "overview");

    active.set("details".to_string());
    tabs.on_event(&SystemEvent::FocusIn);
    assert_eq!(tabs.current_key(), Some("details"));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestPage {
    Overview,
    Details,
    Missing,
}

impl std::fmt::Display for TestPage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Overview => "overview",
            Self::Details => "details",
            Self::Missing => "missing",
        })
    }
}

#[test]
fn controlled_tabs_keep_typed_keys_in_state_and_string_keys_in_snapshots() {
    let active = State::new(TestPage::Details);
    let mut tabs = Tabs::controlled(
        [
            ("Overview", TestPage::Overview),
            ("Details", TestPage::Details),
        ],
        &active,
    );

    assert_eq!(tabs.current_key(), Some("details"));
    tabs.on_event(&key_event(KeyCode::Left));
    assert_eq!(active.get(), TestPage::Overview);
    assert!(matches!(
        tabs.snapshot_fields(),
        SnapshotFields::Tabs {
            active_key: Some(key),
            active_index: 0,
            ..
        } if key == "overview"
    ));
}

#[test]
fn controlled_tabs_recover_from_an_unmatched_value_on_navigation() {
    let active = State::new(TestPage::Missing);
    let mut tabs = Tabs::controlled(
        [
            ("Overview", TestPage::Overview),
            ("Details", TestPage::Details),
        ],
        &active,
    );

    assert_eq!(tabs.current_key(), None);
    tabs.on_event(&key_event(KeyCode::Right));
    assert_eq!(tabs.current_key(), Some("overview"));
    assert_eq!(active.get(), TestPage::Overview);
}

#[test]
fn uncontrolled_tabs_reconcile_preserves_the_active_key_across_reordering() {
    let mut tabs = Tabs::new()
        .tab("Overview", "overview")
        .tab("Details", "details")
        .active(1);

    tabs.sync_from(
        Tabs::new()
            .tab("Details", "details")
            .tab("Overview", "overview"),
    );

    assert_eq!(tabs.active_index(), 0);
    assert_eq!(tabs.current_key(), Some("details"));
}

#[test]
fn external_tab_state_requests_reconcile_and_updates_the_live_component() {
    let active = State::new("overview".to_string());
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(
            Tabs::new()
                .tab("Overview", "overview")
                .tab("Details", "details")
                .active_key(&active),
        )
    }));
    let root = tree.root_id().expect("tabs root");
    tree.reset_invalidation();

    active.set("details".to_string());
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| {
        ViewNode::leaf(
            Tabs::new()
                .tab("Overview", "overview")
                .tab("Details", "details")
                .active_key(&active),
        )
    });
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let tabs = tree
        .get(root)
        .expect("live tabs")
        .component()
        .as_any()
        .downcast_ref::<Tabs>()
        .expect("Tabs component");
    assert_eq!(tabs.current_key(), Some("details"));
}
