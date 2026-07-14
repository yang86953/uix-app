use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::Segmented;

#[test]
fn bound_segmented_writes_keyboard_changes_and_skips_disabled_options() {
    let value = State::new("Day".to_string());
    let mut segmented = Segmented::new(["Day", "Week", "Month"])
        .value(&value)
        .disable_option(1);

    assert_eq!(segmented.current_value().as_deref(), Some("Day"));
    assert_eq!(
        segmented.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(segmented.current_value().as_deref(), Some("Month"));
    assert_eq!(segmented.current_index(), Some(2));
    assert_eq!(value.get(), "Month");

    value.set("Day".to_string());
    segmented.sync_from(
        Segmented::new(["Day", "Week", "Month"])
            .value(&value)
            .disable_option(1),
    );
    assert_eq!(segmented.current_index(), Some(0));
}

#[test]
fn unmatched_controlled_value_moves_to_first_enabled_option() {
    let value = State::new("Unknown".to_string());
    let mut segmented = Segmented::new(["Day", "Week", "Month"])
        .value(&value)
        .disable_option(0);
    assert_eq!(segmented.current_value(), None);

    let _ = segmented.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });

    assert_eq!(segmented.current_value().as_deref(), Some("Week"));
    assert_eq!(value.get(), "Week");
}

#[test]
fn uncontrolled_segmented_keeps_runtime_selection_across_reconcile() {
    let mut segmented = Segmented::new(["Day", "Week", "Month"]).default_selected(1);
    let _ = segmented.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(segmented.current_value().as_deref(), Some("Month"));

    segmented.sync_from(Segmented::new(["Day", "Week", "Month", "Year"]));
    assert_eq!(segmented.current_value().as_deref(), Some("Month"));
}

#[test]
fn external_segmented_state_reconciles_and_invalidates_the_bound_node() {
    let value = State::new("Day".to_string());
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Segmented::new(["Day", "Week"]).value(&value))
    }));
    let root = tree.root_id().expect("segmented root");
    tree.reset_invalidation();

    value.set("Week".to_string());
    assert!(tree.take_reconcile_requested());
    let next =
        ViewAdapter::capture_root(|| ViewNode::leaf(Segmented::new(["Day", "Week"]).value(&value)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let segmented = tree
        .get(root)
        .expect("segmented node")
        .component()
        .as_any()
        .downcast_ref::<Segmented>()
        .expect("Segmented component");
    assert_eq!(segmented.current_value().as_deref(), Some("Week"));
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}
