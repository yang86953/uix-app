use super::*;
use crate::core::{ComponentId, Rect, WindowId};
use crate::ui::core::widget::{WidgetCore, WidgetNode};
use crate::ui::state::{Computed, State};
use crate::ui::widgets::{
    Button, Checkbox, Input, InputNumber, Label, ProgressBar, Select, Slider, Switch,
};
use crate::ui::{AppState, ComponentHandle, SnapshotFields, WidgetTree};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[test]
fn handler_table_bubbles_until_stopped() {
    let child = ComponentId::new(2);
    let parent = ComponentId::new(1);
    let called_child = Rc::new(Cell::new(false));
    let called_parent = Rc::new(Cell::new(false));

    let mut table = HandlerTable::new();
    {
        let called_child = called_child.clone();
        table.on(child, SemanticKind::Click, move |event| {
            called_child.set(true);
            event.stop_propagation();
        });
    }
    {
        let called_parent = called_parent.clone();
        table.on(parent, SemanticKind::Click, move |_| {
            called_parent.set(true);
        });
    }

    let mut event = SemanticEvent::click(
        child,
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::zero(),
            modifiers: KeyMod::NONE,
        },
    );
    let result = table.dispatch_path(&[child, parent], &mut event);

    assert_eq!(result, EventResult::Handled);
    assert!(called_child.get());
    assert!(!called_parent.get());
}

#[test]
fn semantic_event_prevent_default_sets_default_prevented() {
    let id = ComponentId::new(1);
    let mut event = SemanticEvent::click(
        id,
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::zero(),
            modifiers: KeyMod::NONE,
        },
    );

    assert!(!event.default_prevented());

    event.prevent_default();

    assert!(event.default_prevented());
}

#[test]
fn handler_table_once_removes_after_first_dispatch() {
    let id = ComponentId::new(1);
    let calls = Rc::new(Cell::new(0));
    let mut table = HandlerTable::new();
    let calls_for_handler = calls.clone();
    table.register(
        id,
        HandlerRegistration::with_options(
            SemanticKind::Click,
            HandlerOptions::once(),
            Box::new(move |_| calls_for_handler.set(calls_for_handler.get() + 1)),
        ),
    );

    for _ in 0..2 {
        let mut event = SemanticEvent::click(
            id,
            ClickEvent {
                button: MouseButton::Left,
                pos: Point::zero(),
                modifiers: KeyMod::NONE,
            },
        );
        let _ = table.dispatch_path(&[id], &mut event);
    }

    assert_eq!(calls.get(), 1);
}

#[test]
fn handler_table_when_is_evaluated_per_dispatch() {
    let id = ComponentId::new(1);
    let enabled = Rc::new(Cell::new(false));
    let calls = Rc::new(Cell::new(0));
    let mut table = HandlerTable::new();
    let enabled_for_predicate = enabled.clone();
    let calls_for_handler = calls.clone();
    table.register(
        id,
        HandlerRegistration::with_options(
            SemanticKind::Click,
            HandlerOptions::when(move |_| enabled_for_predicate.get()),
            Box::new(move |_| calls_for_handler.set(calls_for_handler.get() + 1)),
        ),
    );

    let mut event = SemanticEvent::click(
        id,
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::zero(),
            modifiers: KeyMod::NONE,
        },
    );
    let _ = table.dispatch_path(&[id], &mut event);
    enabled.set(true);
    let mut event = SemanticEvent::click(
        id,
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::zero(),
            modifiers: KeyMod::NONE,
        },
    );
    let _ = table.dispatch_path(&[id], &mut event);

    assert_eq!(calls.get(), 1);
}

#[test]
fn handler_table_keys_by_component_id_generation() {
    let first_generation = ComponentId::from_parts(7, 0);
    let second_generation = ComponentId::from_parts(7, 1);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut table = HandlerTable::new();

    {
        let calls = calls.clone();
        table.on(first_generation, SemanticKind::Click, move |_| {
            calls.borrow_mut().push("first");
        });
    }
    {
        let calls = calls.clone();
        table.on(second_generation, SemanticKind::Click, move |_| {
            calls.borrow_mut().push("second");
        });
    }

    let mut event = SemanticEvent::click(
        second_generation,
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::zero(),
            modifiers: KeyMod::NONE,
        },
    );
    let result = table.dispatch_path(&[second_generation], &mut event);

    assert_eq!(result, EventResult::Handled);
    assert_eq!(calls.borrow().as_slice(), &["second"]);
}

#[test]
fn plain_handler_registration_has_no_stable_authored_signature() {
    let registration = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}));

    let signature = registration.authored_signature();

    assert_eq!(signature.kind, SemanticKind::Click);
    assert_eq!(signature.generation, None);
    assert_eq!(signature.capture_fingerprint, None);
}

#[test]
fn explicit_handler_generation_authors_stable_signature() {
    let signature = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_generation(7)
        .authored_signature();

    assert_eq!(signature.kind, SemanticKind::Click);
    assert_eq!(signature.generation, Some(7));
    assert_eq!(signature.capture_fingerprint, None);
}

#[test]
fn state_capture_authors_stable_signature_by_state_identity() {
    let state = State::new(1);
    let first = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_state_capture(&state)
        .authored_signature();

    state.set(2);

    let same_state = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_state_capture(&state)
        .authored_signature();
    let other_state = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_state_capture(&State::new(2))
        .authored_signature();

    assert_eq!(first.generation, Some(0));
    assert_eq!(first.capture_fingerprint, same_state.capture_fingerprint);
    assert_ne!(first.capture_fingerprint, other_state.capture_fingerprint);
}

#[test]
fn computed_capture_authors_stable_signature_by_computed_identity() {
    let source = State::new(1);
    let source_for_computed = source.clone();
    let computed = Computed::new(move || source_for_computed.get() * 2);
    let first = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_computed_capture(&computed)
        .authored_signature();

    source.set(2);
    assert_eq!(computed.get(), 4);

    let same_computed = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_computed_capture(&computed)
        .authored_signature();
    let other_computed = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_computed_capture(&Computed::new(|| 4))
        .authored_signature();

    assert_eq!(first.generation, Some(0));
    assert_eq!(first.capture_fingerprint, same_computed.capture_fingerprint);
    assert_ne!(
        first.capture_fingerprint,
        other_computed.capture_fingerprint
    );
}

#[test]
fn window_capture_authors_stable_signature_by_window_id() {
    let first = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_window_capture(WindowId::new(7))
        .authored_signature();
    let same = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_window_capture(WindowId::new(7))
        .authored_signature();
    let different = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_window_capture(WindowId::new(8))
        .authored_signature();

    assert_eq!(first.generation, Some(0));
    assert_eq!(first.capture_fingerprint, same.capture_fingerprint);
    assert_ne!(first.capture_fingerprint, different.capture_fingerprint);
}

#[test]
fn multiple_captures_author_stable_order_independent_signature() {
    let state = State::new(1);
    let state_then_window = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_state_capture(&state)
        .with_window_capture(WindowId::new(7))
        .authored_signature();
    let window_then_state = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_window_capture(WindowId::new(7))
        .with_state_capture(&state)
        .authored_signature();
    let different_window = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
        .with_state_capture(&state)
        .with_window_capture(WindowId::new(8))
        .authored_signature();

    assert_eq!(state_then_window.generation, Some(0));
    assert_eq!(
        state_then_window.capture_fingerprint,
        window_then_state.capture_fingerprint
    );
    assert_ne!(
        state_then_window.capture_fingerprint,
        different_window.capture_fingerprint
    );
}

#[test]
fn semantic_handler_macro_authors_state_fingerprint_and_dispatches() {
    let state = State::new(1);
    let calls = State::new(0);
    let mut registration = crate::semantic_handler!(
        SemanticKind::Click,
        state [state, calls],
        |event| {
            calls.set(calls.get() + state.get());
            event.stop_propagation();
        }
    );
    let signature = registration.authored_signature();

    assert_eq!(signature.kind, SemanticKind::Click);
    assert_eq!(signature.generation, Some(0));
    assert!(signature.capture_fingerprint.is_some());

    let id = ComponentId::new(9);
    let mut event = SemanticEvent::click(
        id,
        ClickEvent {
            button: MouseButton::Left,
            pos: Point::zero(),
            modifiers: KeyMod::NONE,
        },
    );
    (registration.handler)(&mut event);

    assert_eq!(calls.get(), 1);
    assert!(event.propagation_stopped());
}

#[test]
fn semantic_handler_macro_combines_computed_and_window_fingerprints() {
    let source = State::new(2);
    let source_for_computed = source.clone();
    let computed = Computed::new(move || source_for_computed.get() * 2);
    let window_id = WindowId::new(12);
    let first = crate::semantic_handler!(
        SemanticKind::Change,
        computed[computed],
        window[window_id],
        |_event| {}
    )
    .authored_signature();

    source.set(3);
    let same = crate::semantic_handler!(
        SemanticKind::Change,
        computed[computed],
        window[window_id],
        |_event| {}
    )
    .authored_signature();
    let other_window = WindowId::new(13);
    let different = crate::semantic_handler!(
        SemanticKind::Change,
        computed[computed],
        window[other_window],
        |_event| {}
    )
    .authored_signature();

    assert_eq!(first.generation, Some(0));
    assert_eq!(first.capture_fingerprint, same.capture_fingerprint);
    assert_ne!(first.capture_fingerprint, different.capture_fingerprint);
}

#[test]
fn widget_node_on_semantic_capture_registers_captured_handler() {
    let state = State::new(1);
    let calls = Rc::new(Cell::new(0));
    let calls_for_handler = calls.clone();
    let node = WidgetNode::leaf(Box::new(Label::new("captured"))).on_semantic_capture(
        HandlerRegistration::new(
            SemanticKind::Change,
            Box::new(move |_| calls_for_handler.set(calls_for_handler.get() + 1)),
        ),
        &state,
    );
    let mut tree = WidgetTree::new();
    let root = tree.build(node);

    state.set(2);
    let _ = tree.dispatch_semantic(SemanticEvent::change(root, "next"));

    assert_eq!(calls.get(), 1);
}

#[test]
fn widget_node_on_semantic_computed_capture_registers_captured_handler() {
    let source = State::new(1);
    let source_for_computed = source.clone();
    let computed = Computed::new(move || source_for_computed.get() * 2);
    let calls = Rc::new(Cell::new(0));
    let calls_for_handler = calls.clone();
    let node = WidgetNode::leaf(Box::new(Label::new("captured"))).on_semantic_computed_capture(
        HandlerRegistration::new(
            SemanticKind::Change,
            Box::new(move |_| calls_for_handler.set(calls_for_handler.get() + 1)),
        ),
        &computed,
    );
    let mut tree = WidgetTree::new();
    let root = tree.build(node);

    source.set(2);
    assert_eq!(computed.get(), 4);
    let _ = tree.dispatch_semantic(SemanticEvent::change(root, "next"));

    assert_eq!(calls.get(), 1);
}

#[test]
fn widget_node_on_semantic_window_capture_registers_captured_handler() {
    let calls = Rc::new(Cell::new(0));
    let calls_for_handler = calls.clone();
    let node = WidgetNode::leaf(Box::new(Label::new("captured"))).on_semantic_window_capture(
        HandlerRegistration::new(
            SemanticKind::Change,
            Box::new(move |_| calls_for_handler.set(calls_for_handler.get() + 1)),
        ),
        WindowId::new(7),
    );
    let mut tree = WidgetTree::new();
    let root = tree.build(node);

    let _ = tree.dispatch_semantic(SemanticEvent::change(root, "next"));

    assert_eq!(calls.get(), 1);
}

#[derive(Debug, PartialEq)]
struct BusinessPayload {
    value: u32,
}

#[test]
fn register_semantic_macro_matches_custom_event_kind() {
    let event = SemanticEvent::custom(ComponentId::new(1), BusinessPayload { value: 7 });

    assert_eq!(event.kind, register_semantic!(BusinessPayload));
}

#[test]
fn handler_table_dispatches_typed_custom_payload() {
    let id = ComponentId::new(1);
    let value = Rc::new(Cell::new(0));
    let value_for_handler = value.clone();
    let mut table = HandlerTable::new();
    table.on_custom::<BusinessPayload>(id, move |payload| {
        value_for_handler.set(payload.value);
    });

    let mut event = SemanticEvent::custom(ComponentId::new(1), BusinessPayload { value: 42 });
    let result = table.dispatch_path(&[id], &mut event);

    assert_eq!(result, EventResult::Handled);
    assert_eq!(value.get(), 42);
}

#[test]
fn component_handle_emit_uses_semantic_bubble_path() {
    let tree = Rc::new(RefCell::new(WidgetTree::new()));
    let root = tree.borrow_mut().set_root(Box::new(Button::new("root")));
    let child = tree
        .borrow_mut()
        .add_child(root, Box::new(Button::new("child")));

    let calls = Rc::new(RefCell::new(Vec::new()));
    {
        let calls = calls.clone();
        tree.borrow_mut()
            .handler_table()
            .on(child, SemanticKind::Change, move |event| {
                calls
                    .borrow_mut()
                    .push((event.target, event.current_target, "child"));
            });
    }
    {
        let calls = calls.clone();
        tree.borrow_mut()
            .handler_table()
            .on(root, SemanticKind::Change, move |event| {
                calls
                    .borrow_mut()
                    .push((event.target, event.current_target, "root"));
            });
    }

    let handle = ComponentHandle::new(child, &tree);
    let result = handle.emit(SemanticEvent::change(root, "from-handle"));

    assert_eq!(result, EventResult::Handled);
    assert_eq!(
        calls.borrow().as_slice(),
        &[(child, child, "child"), (child, root, "root")]
    );
}

#[test]
fn app_state_lookup_handle_emit_drains_into_widget_tree() {
    let mut tree = WidgetTree::new();
    let app_state = AppState::new();
    let root = tree.set_root(Box::new(Button::new("root")));
    let child = tree.add_child(root, Box::new(Button::new("child")));

    let calls = Rc::new(RefCell::new(Vec::new()));
    {
        let calls = calls.clone();
        tree.handler_table()
            .on(child, SemanticKind::Change, move |event| {
                calls
                    .borrow_mut()
                    .push((event.target, event.current_target, "child"));
            });
    }
    {
        let calls = calls.clone();
        tree.handler_table()
            .on(root, SemanticKind::Change, move |event| {
                calls
                    .borrow_mut()
                    .push((event.target, event.current_target, "root"));
            });
    }

    tree.layout();
    tree.set_app_state(app_state.clone());
    let handle = app_state.get_handle(child).unwrap();
    assert_eq!(handle.label().as_deref(), Some("child"));

    assert_eq!(
        handle.emit(SemanticEvent::change(root, "from-lookup")),
        EventResult::Handled
    );
    assert!(calls.borrow().is_empty());
    assert!(tree.drain_app_state_semantic_events());
    assert_eq!(
        calls.borrow().as_slice(),
        &[(child, child, "child"), (child, root, "root")]
    );
}

#[test]
fn app_state_lookup_handle_exposes_read_only_config_getters() {
    let mut tree = WidgetTree::new();
    let app_state = AppState::new();
    let root = tree.set_root(Box::new(Select::new().placeholder("Pick").disabled(true)));
    let checkbox = tree.add_child(root, Box::new(Checkbox::new("Agree").checked(true)));
    let input_number = tree.add_child(
        root,
        Box::new(InputNumber::new("Amount").value(12.5).disabled(true)),
    );

    tree.layout();
    tree.set_app_state(app_state.clone());

    let select = app_state.get_handle(root).unwrap();
    assert_eq!(select.placeholder().as_deref(), Some("Pick"));
    assert_eq!(select.disabled(), Some(true));

    let checkbox = app_state.get_handle(checkbox).unwrap();
    assert_eq!(checkbox.checked(), Some(true));
    assert_eq!(checkbox.disabled(), Some(false));

    let input_number = app_state.get_handle(input_number).unwrap();
    assert_eq!(input_number.placeholder().as_deref(), Some("Amount"));
    assert_eq!(input_number.disabled(), Some(true));
    assert_eq!(input_number.numeric_value(), Some(12.5));
}

#[test]
fn app_state_lookup_emit_is_retained_for_target_widget_tree() {
    let app_state = AppState::new();
    let mut unrelated_tree = WidgetTree::new();
    unrelated_tree.set_root(Box::new(Button::new("unrelated")));
    unrelated_tree.layout();
    unrelated_tree.set_app_state(app_state.clone());

    let mut target_tree = WidgetTree::new();
    let target_root = target_tree.set_root(Box::new(Button::new("target-root")));
    let target_child = target_tree.add_child(target_root, Box::new(Button::new("target-child")));
    let calls = Rc::new(RefCell::new(Vec::new()));
    {
        let calls = calls.clone();
        target_tree
            .handler_table()
            .on(target_child, SemanticKind::Change, move |event| {
                calls
                    .borrow_mut()
                    .push((event.target, event.current_target));
            });
    }
    target_tree.layout();
    target_tree.set_app_state(app_state.clone());

    let handle = app_state
        .get_handle(target_child)
        .expect("target child should be registered");
    assert_eq!(
        handle.emit(SemanticEvent::change(target_child, "from-lookup")),
        EventResult::Handled
    );

    assert!(!unrelated_tree.drain_app_state_semantic_events());
    assert!(calls.borrow().is_empty());
    assert!(target_tree.drain_app_state_semantic_events());
    assert_eq!(calls.borrow().as_slice(), &[(target_child, target_child)]);
}

#[test]
fn app_state_get_handle_panics_off_owner_thread() {
    let mut tree = WidgetTree::new();
    let app_state = AppState::new();
    let root = tree.set_root(Box::new(Button::new("root")));

    tree.layout();
    tree.set_app_state(app_state.clone());

    let result = std::thread::spawn(move || {
        let _ = app_state.get_handle(root).is_some();
    })
    .join();

    assert!(result.is_err());
}

#[test]
fn lookup_component_handle_emit_panics_off_owner_thread() {
    let mut tree = WidgetTree::new();
    let app_state = AppState::new();
    let root = tree.set_root(Box::new(Button::new("root")));

    tree.layout();
    tree.set_app_state(app_state.clone());
    let app_state_inner = std::sync::Arc::downgrade(&app_state.inner);

    let result = std::thread::spawn(move || {
        let handle = ComponentHandle::from_app_state(root, app_state_inner);
        let _ = handle.emit(SemanticEvent::change(root, "off-thread"));
    })
    .join();

    assert!(result.is_err());
}

#[test]
fn component_handle_invalidate_marks_narrow_paint_only() {
    let tree = Rc::new(RefCell::new(WidgetTree::new()));
    let root = tree.borrow_mut().set_root(Box::new(Button::new("root")));
    let child = tree
        .borrow_mut()
        .add_child(root, Box::new(Button::new("child")));

    {
        let mut tree = tree.borrow_mut();
        tree.get_mut(root)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 120.0, 40.0));
        tree.get_mut(child)
            .unwrap()
            .set_frame(Rect::new(12.0, 8.0, 32.0, 18.0));
        tree.reset_invalidation();
    }

    ComponentHandle::new(child, &tree).invalidate();

    let tree = tree.borrow();
    let invalidation = tree
        .invalidation()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    assert!(invalidation.node_needs_paint(child));
    assert!(!invalidation.node_needs_paint(root));
    assert!(!invalidation.has_layout());
    drop(invalidation);
    assert!(tree.has_render_work());
    assert!(!tree.dirty_region().full_frame);
}

#[test]
fn component_handle_snapshot_exposes_read_only_config_getters() {
    let tree = Rc::new(RefCell::new(WidgetTree::new()));
    let root = tree
        .borrow_mut()
        .set_root(Box::new(Button::new("Save").disabled(true)));
    let input = tree
        .borrow_mut()
        .add_child(root, Box::new(Input::new("Email").disabled(true)));

    let button = ComponentHandle::new(root, &tree);
    assert_eq!(button.text().as_deref(), Some("Save"));
    assert_eq!(button.label().as_deref(), Some("Save"));
    assert_eq!(button.disabled(), Some(true));
    assert_eq!(button.placeholder(), None);
    assert!(matches!(
        button.snapshot_fields(),
        Some(SnapshotFields::Button {
            text,
            disabled: true,
            ..
        }) if text == "Save"
    ));

    let input = ComponentHandle::new(input, &tree);
    assert_eq!(input.text(), None);
    assert_eq!(input.placeholder().as_deref(), Some("Email"));
    assert_eq!(input.disabled(), Some(true));
}

#[test]
fn component_handle_getters_cover_common_snapshot_fields() {
    let tree = Rc::new(RefCell::new(WidgetTree::new()));
    let root = tree
        .borrow_mut()
        .set_root(Box::new(Select::new().placeholder("Pick").disabled(true)));
    let checkbox = tree
        .borrow_mut()
        .add_child(root, Box::new(Checkbox::new("Agree").checked(true)));
    let switch = tree
        .borrow_mut()
        .add_child(root, Box::new(Switch::new().checked(true).disabled(true)));
    let slider = tree
        .borrow_mut()
        .add_child(root, Box::new(Slider::new().value(42.0)));
    let input_number = tree.borrow_mut().add_child(
        root,
        Box::new(InputNumber::new("Amount").value(12.5).disabled(true)),
    );
    let progress = tree
        .borrow_mut()
        .add_child(root, Box::new(ProgressBar::new().progress(0.75)));

    let select = ComponentHandle::new(root, &tree);
    assert_eq!(select.placeholder().as_deref(), Some("Pick"));
    assert_eq!(select.disabled(), Some(true));

    let checkbox = ComponentHandle::new(checkbox, &tree);
    assert_eq!(checkbox.checked(), Some(true));
    assert_eq!(checkbox.disabled(), Some(false));

    let switch = ComponentHandle::new(switch, &tree);
    assert_eq!(switch.checked(), Some(true));
    assert_eq!(switch.disabled(), Some(true));

    assert_eq!(
        ComponentHandle::new(slider, &tree).numeric_value(),
        Some(42.0)
    );
    let input_number = ComponentHandle::new(input_number, &tree);
    assert_eq!(input_number.placeholder().as_deref(), Some("Amount"));
    assert_eq!(input_number.disabled(), Some(true));
    assert_eq!(input_number.numeric_value(), Some(12.5));
    assert_eq!(
        ComponentHandle::new(progress, &tree).numeric_value(),
        Some(0.75)
    );
}

#[test]
fn component_handle_snapshot_is_unavailable_after_unmount_or_during_active_borrow() {
    let tree = Rc::new(RefCell::new(WidgetTree::new()));
    let root = tree.borrow_mut().set_root(Box::new(Button::new("Save")));
    let handle = ComponentHandle::new(root, &tree);

    let active_borrow = tree.borrow_mut();
    assert!(handle.snapshot().is_none());
    drop(active_borrow);

    tree.borrow_mut().remove(root);
    assert!(!handle.is_alive());
    assert!(handle.snapshot().is_none());
    assert_eq!(handle.text(), None);
}

#[test]
fn component_handle_snapshot_excludes_button_interaction_state() {
    let tree = Rc::new(RefCell::new(WidgetTree::new()));
    let root = tree.borrow_mut().set_root(Box::new(Button::new("Save")));
    let handle = ComponentHandle::new(root, &tree);
    let before = handle.snapshot_fields();

    let event = SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    let _ = tree.borrow_mut().dispatch_event(&event);

    assert_eq!(handle.snapshot_fields(), before);
}
