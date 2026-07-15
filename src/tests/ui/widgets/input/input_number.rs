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

#[test]
fn input_number_accepts_platform_text_and_enter_commits_the_buffer() {
    let value = State::new(4.0f64);
    let mut input = InputNumber::new().min(0.0).max(10.0).value(&value);
    assert!(input
        .as_text_input()
        .expect("InputNumber text input capability")
        .accepts_text_input());

    let _ = input.on_event(&SystemEvent::FocusIn);
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    let _ = input.on_event(&SystemEvent::TextInput {
        text: "7.5".to_owned(),
    });
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });

    assert_eq!(input.current_value(), 7.5);
    assert_eq!(value.get(), 7.5);
}

#[test]
fn input_number_focus_out_commits_and_invalid_text_preserves_value() {
    let value = State::new(2.0f64);
    let mut input = InputNumber::new().value(&value);
    let _ = input.on_event(&SystemEvent::FocusIn);
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    let _ = input.on_event(&SystemEvent::TextInput {
        text: "8".to_owned(),
    });
    let _ = input.on_event(&SystemEvent::FocusOut);
    assert_eq!(value.get(), 8.0);

    let _ = input.on_event(&SystemEvent::FocusIn);
    let _ = input.on_event(&SystemEvent::TextInput {
        text: "..".to_owned(),
    });
    let _ = input.on_event(&SystemEvent::FocusOut);
    assert_eq!(value.get(), 8.0);
}

#[test]
fn input_number_pointer_step_buttons_use_the_configured_step() {
    let value = State::new(4i32);
    let mut input = InputNumber::new().step(2.0).value(&value);
    let button_x = 60.0;

    assert_eq!(
        input.on_event(&SystemEvent::PointerDown {
            pos: Point::new(button_x, 5.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(value.get(), 6);

    let _ = input.on_event(&SystemEvent::PointerDown {
        pos: Point::new(button_x, 25.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(value.get(), 4);
}

#[test]
fn disabled_input_number_does_not_request_text_input_or_handle_steps() {
    let mut input = InputNumber::new().disabled(true);
    assert!(!input
        .as_text_input()
        .expect("InputNumber text input capability")
        .accepts_text_input());
    assert_eq!(
        input.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Up,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}
