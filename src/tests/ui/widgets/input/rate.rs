use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::Rate;
use crate::ui::{with_config, ComponentConfig};

#[test]
fn bound_rate_writes_keyboard_changes_and_reads_external_updates() {
    let value = State::new(3u32);
    let mut rate = Rate::new().count(5).value(&value);

    assert_eq!(rate.current_value(), 3);
    assert_eq!(
        rate.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(rate.current_value(), 4);
    assert_eq!(value.get(), 4);

    value.set(10);
    rate.sync_from(Rate::new().count(5).value(&value));
    assert_eq!(rate.current_value(), 5);

    let _ = rate.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(rate.current_value(), 4);
    assert_eq!(value.get(), 4);
}

#[test]
fn half_rate_uses_half_step_units_for_state_values() {
    let value = State::new(7u32);
    let mut rate = Rate::new().value(&value).count(4).allow_half();

    assert_eq!(rate.current_value(), 7);

    let _ = rate.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });

    assert_eq!(rate.current_value(), 8);
    assert_eq!(value.get(), 8);
}

#[test]
fn uncontrolled_rate_keeps_runtime_value_across_reconcile() {
    let mut rate = Rate::new().count(5).default_value(2);
    let _ = rate.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(rate.current_value(), 3);

    rate.sync_from(Rate::new().count(4).default_value(1));
    assert_eq!(rate.current_value(), 3);
}

#[test]
fn external_rate_state_reconciles_and_invalidates_the_bound_node() {
    let value = State::new(2u32);
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Rate::new().count(5).value(&value))
    }));
    let root = tree.root_id().expect("rate root");
    tree.reset_invalidation();

    value.set(4);
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| ViewNode::leaf(Rate::new().count(5).value(&value)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let rate = tree
        .get(root)
        .expect("rate node")
        .component()
        .as_any()
        .downcast_ref::<Rate>()
        .expect("Rate component");
    assert_eq!(rate.current_value(), 4);
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}

#[test]
fn provider_size_and_explicit_override_drive_rate_layout_and_half_hit() {
    let large = ComponentConfig::new().component_size(ControlSize::Large);
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let mut rate = with_config(&large, || Rate::new().count(2).allow_half());
    assert_eq!(rate.measure(max).h, 40.0);

    assert_eq!(
        rate.on_event(&SystemEvent::PointerDown {
            pos: Point::new(14.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(rate.current_value(), 1);

    let small = with_config(&large, || Rate::new().size(ControlSize::Small));
    assert_eq!(small.measure(max).h, 24.0);
}
