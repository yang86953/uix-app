use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::Slider;

#[test]
fn bound_slider_writes_keyboard_changes_and_reads_external_updates() {
    let value = State::new(4.0);
    let mut slider = Slider::new(0.0..=10.0).step(2.0).value(&value);

    assert_eq!(slider.current_value(), 4.0);
    assert_eq!(
        slider.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(slider.current_value(), 6.0);
    assert_eq!(value.get(), 6.0);

    value.set(20.0);
    slider.sync_from(Slider::new(0.0..=10.0).step(2.0).value(&value));
    assert_eq!(slider.current_value(), 10.0);

    let _ = slider.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(slider.current_value(), 8.0);
    assert_eq!(value.get(), 8.0);
}

#[test]
fn uncontrolled_slider_keeps_runtime_value_across_reconcile() {
    let mut slider = Slider::new(1.0..=10.0).step(2.0).default_value(1.0);
    let _ = slider.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(slider.current_value(), 3.0);

    slider.sync_from(Slider::new(-10.0..=10.0).step(0.5).default_value(-5.0));
    assert_eq!(slider.current_value(), 3.0);
}

#[test]
fn external_slider_state_reconciles_and_invalidates_the_bound_node() {
    let value = State::new(4.0);
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Slider::new(0.0..=10.0).value(&value))
    }));
    let root = tree.root_id().expect("slider root");
    tree.reset_invalidation();

    value.set(7.0);
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| ViewNode::leaf(Slider::new(0.0..=10.0).value(&value)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let slider = tree
        .get(root)
        .expect("slider node")
        .component()
        .as_any()
        .downcast_ref::<Slider>()
        .expect("Slider component");
    assert_eq!(slider.current_value(), 7.0);
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}
