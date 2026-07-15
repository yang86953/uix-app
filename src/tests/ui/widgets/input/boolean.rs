use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Checkbox, Switch};
use crate::ui::{with_config, ComponentConfig};

#[test]
fn checkbox_binding_writes_user_changes_and_reads_external_updates() {
    let checked = State::new(false);
    let mut checkbox = Checkbox::new("Agree").checked(&checked);

    assert_eq!(
        checkbox.on_event(&SystemEvent::PointerDown {
            pos: Point::new(1.0, 1.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(checkbox.is_checked());
    assert!(checked.get());

    checked.set(false);
    checkbox.sync_from(Checkbox::new("Agree").checked(&checked));
    assert!(!checkbox.is_checked());
}

#[test]
fn switch_binding_writes_keyboard_changes_and_preserves_uncontrolled_runtime_value() {
    let checked = State::new(false);
    let mut switch = Switch::new().checked(&checked);

    assert_eq!(
        switch.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(switch.is_checked());
    assert!(checked.get());

    let mut uncontrolled = Switch::new().default_checked(true);
    let _ = uncontrolled.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Space,
        mods: KeyMod::NONE,
    });
    uncontrolled.sync_from(Switch::new().default_checked(true));
    assert!(!uncontrolled.is_checked());
}

#[test]
fn external_boolean_state_reconciles_and_invalidates_the_bound_node() {
    let checked = State::new(false);
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Checkbox::new("Agree").checked(&checked))
    }));
    let root = tree.root_id().expect("checkbox root");
    tree.reset_invalidation();

    checked.set(true);
    assert!(tree.take_reconcile_requested());
    let next =
        ViewAdapter::capture_root(|| ViewNode::leaf(Checkbox::new("Agree").checked(&checked)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let checkbox = tree
        .get(root)
        .expect("checkbox node")
        .component()
        .as_any()
        .downcast_ref::<Checkbox>()
        .expect("Checkbox component");
    assert!(checkbox.is_checked());
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}

#[test]
fn provider_size_reaches_boolean_controls_and_explicit_size_wins() {
    let large = ComponentConfig::new().component_size(ControlSize::Large);
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let (checkbox, switch) = with_config(&large, || (Checkbox::new("Agree"), Switch::new()));

    assert_eq!(checkbox.measure(max).h, 40.0);
    assert_eq!(switch.measure(max).h, 40.0);

    let (checkbox, switch) = with_config(&large, || {
        (
            Checkbox::new("Agree").size(ControlSize::Small),
            Switch::new().size(ControlSize::Small),
        )
    });
    assert_eq!(checkbox.measure(max).h, 24.0);
    assert_eq!(switch.measure(max).h, 24.0);
}
