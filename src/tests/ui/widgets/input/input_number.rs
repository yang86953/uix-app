use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::InputNumber;

#[test]
fn bound_integer_value_writes_keyboard_changes_and_reads_external_updates() {
    let value = State::new(4i32);
    let mut input = InputNumber::new()
        .placeholder("Count")
        .min(0.0)
        .max(10.0)
        .step(2.0)
        .value(&value);

    assert_eq!(input.current_value(), 4.0);
    assert_eq!(
        input.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Up,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(input.current_value(), 6.0);
    assert_eq!(value.get(), 6);

    value.set(20);
    input.sync_from(
        InputNumber::new()
            .placeholder("Count")
            .min(0.0)
            .max(10.0)
            .step(2.0)
            .value(&value),
    );
    assert_eq!(input.current_value(), 10.0);

    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    assert_eq!(input.current_value(), 8.0);
    assert_eq!(value.get(), 8);
}

#[test]
fn integer_binding_normalizes_fractional_steps_before_publishing() {
    let value = State::new(1i32);
    let mut input = InputNumber::new().step(0.6).value(&value);

    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Up,
        mods: KeyMod::NONE,
    });

    assert_eq!(input.current_value(), 2.0);
    assert_eq!(value.get(), 2);
}

#[test]
fn uncontrolled_value_survives_reconcile() {
    let mut input = InputNumber::new()
        .placeholder("Count")
        .min(0.0)
        .max(100.0)
        .step(1.0);
    let _ = input.on_event(&SystemEvent::PointerDown {
        pos: Point::new(1.0, 1.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Up,
        mods: KeyMod::NONE,
    });

    input.sync_from(
        InputNumber::new()
            .placeholder("Count")
            .min(0.0)
            .max(100.0)
            .step(1.0),
    );

    assert_eq!(input.current_value(), 1.0);
}

#[test]
fn external_state_reconciles_and_invalidates_the_bound_node() {
    let value = State::new(4.0);
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(InputNumber::new().min(0.0).max(10.0).value(&value))
    }));
    let root = tree.root_id().expect("input number root");
    tree.reset_invalidation();

    value.set(7.0);
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| {
        ViewNode::leaf(InputNumber::new().min(0.0).max(10.0).value(&value))
    });
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let input = tree
        .get(root)
        .expect("input number node")
        .component()
        .as_any()
        .downcast_ref::<InputNumber>()
        .expect("InputNumber component");
    assert_eq!(input.current_value(), 7.0);
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}
