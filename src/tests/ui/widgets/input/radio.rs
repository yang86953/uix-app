use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::Radio;

#[test]
fn bound_radio_writes_keyboard_changes_and_reads_external_updates() {
    let value = State::new("Medium".to_string());
    let mut radio = Radio::group("size", ["Small", "Medium", "Large"], &value);

    assert_eq!(radio.current_value().as_deref(), Some("Medium"));
    assert_eq!(radio.current_index(), Some(1));
    assert_eq!(
        radio.snapshot_fields().accessibility().name.as_deref(),
        Some("size")
    );
    assert_eq!(
        radio.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(radio.current_value().as_deref(), Some("Large"));
    assert_eq!(value.get(), "Large");

    value.set("Small".to_string());
    radio.sync_from(Radio::group("size", ["Small", "Medium", "Large"], &value));
    assert_eq!(radio.current_index(), Some(0));
}

#[test]
fn unmatched_controlled_value_has_no_selection_until_user_moves() {
    let value = State::new("Unknown".to_string());
    let mut radio = Radio::group("size", ["Small", "Medium"], &value);
    assert_eq!(radio.current_value(), None);

    let _ = radio.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });

    assert_eq!(radio.current_value().as_deref(), Some("Small"));
    assert_eq!(value.get(), "Small");
}

#[test]
fn uncontrolled_radio_keeps_runtime_selection_across_reconcile() {
    let mut radio = Radio::new().options(["A", "B", "C"]).default_selected(1);
    let _ = radio.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(radio.current_value().as_deref(), Some("C"));

    radio.sync_from(Radio::new().options(["A", "B", "C", "D"]));
    assert_eq!(radio.current_value().as_deref(), Some("C"));
}

#[test]
fn external_radio_state_reconciles_and_invalidates_the_bound_node() {
    let value = State::new("A".to_string());
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Radio::group("choice", ["A", "B"], &value))
    }));
    let root = tree.root_id().expect("radio root");
    tree.reset_invalidation();

    value.set("B".to_string());
    assert!(tree.take_reconcile_requested());
    let next =
        ViewAdapter::capture_root(|| ViewNode::leaf(Radio::group("choice", ["A", "B"], &value)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let radio = tree
        .get(root)
        .expect("radio node")
        .component()
        .as_any()
        .downcast_ref::<Radio>()
        .expect("Radio component");
    assert_eq!(radio.current_value().as_deref(), Some("B"));
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}
