use super::*;
use crate::core::{ComponentId, Rect};
use crate::ui::core::widget::{WidgetCore, WidgetNode};
use crate::ui::state::State;
use crate::ui::widgets::{Button, Input, Label};
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
fn widget_node_on_semantic_window_capture_registers_captured_handler() {
    use crate::core::WindowId;

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
        tree.reset_dirty();
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
