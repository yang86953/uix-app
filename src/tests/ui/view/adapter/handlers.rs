use super::*;

#[test]
fn reconcile_preserves_consumed_once_handler_when_signature_is_unchanged() {
    use crate::ui::event::{ClickEvent, HandlerOptions, HandlerRegistration};
    use crate::ui::view::button;
    use crate::ui::view::View;

    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_generation(7),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_generation(7),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn reconcile_reregisters_plain_handler_without_stable_signature() {
    use crate::ui::event::{ClickEvent, HandlerOptions, HandlerRegistration};
    use crate::ui::view::button;
    use crate::ui::view::View;

    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers = vec![HandlerRegistration::with_options(
        SemanticKind::Click,
        HandlerOptions::once(),
        {
            let first_hits = first_hits.clone();
            Box::new(move |_| first_hits.set(first_hits.get() + 1))
        },
    )];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers = vec![HandlerRegistration::with_options(
        SemanticKind::Click,
        HandlerOptions::once(),
        {
            let second_hits = second_hits.clone();
            Box::new(move |_| second_hits.set(second_hits.get() + 1))
        },
    )];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn reconcile_reregisters_handler_when_generation_changes() {
    use crate::ui::event::{ClickEvent, HandlerOptions, HandlerRegistration};
    use crate::ui::view::button;
    use crate::ui::view::View;

    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_generation(7),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_generation(8),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn reconcile_preserves_consumed_once_handler_when_state_capture_fingerprint_is_unchanged() {
    use crate::ui::event::{ClickEvent, HandlerOptions, HandlerRegistration};
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::view::View;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_state_capture(&state),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    state.set(2);
    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_state_capture(&state),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn reconcile_reregisters_handler_when_state_capture_fingerprint_changes() {
    use crate::ui::event::{ClickEvent, HandlerOptions, HandlerRegistration};
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::view::View;

    let first_state = State::new(1);
    let second_state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_state_capture(&first_state),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_state_capture(&second_state),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn reconcile_reregisters_handler_when_options_change_with_same_capture() {
    use crate::ui::event::{ClickEvent, HandlerOptions, HandlerRegistration};
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::view::View;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_state_capture(&state),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers = vec![HandlerRegistration::new(SemanticKind::Click, {
        let second_hits = second_hits.clone();
        Box::new(move |_| second_hits.set(second_hits.get() + 1))
    })
    .with_state_capture(&state)];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn reconcile_preserves_consumed_once_handler_when_window_capture_fingerprint_is_unchanged() {
    use crate::ui::event::{ClickEvent, HandlerOptions, HandlerRegistration};
    use crate::ui::view::button;
    use crate::ui::view::View;

    let window_id = WindowId::new(7);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_window_capture(window_id),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_window_capture(window_id),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn reconcile_reregisters_handler_when_window_capture_fingerprint_changes() {
    use crate::ui::event::{ClickEvent, HandlerOptions, HandlerRegistration};
    use crate::ui::view::button;
    use crate::ui::view::View;

    let first_window = WindowId::new(7);
    let second_window = WindowId::new(8);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_window_capture(first_window),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_window_capture(second_window),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn reconcile_reregisters_handler_when_one_of_multiple_captures_changes() {
    use crate::ui::event::{ClickEvent, HandlerOptions, HandlerRegistration};
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::view::View;

    let first_state = State::new(1);
    let second_state = State::new(1);
    let window_id = WindowId::new(7);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_state_capture(&first_state)
            .with_window_capture(window_id),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_state_capture(&second_state)
            .with_window_capture(window_id),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn button_dsl_state_capture_preserves_handler_when_fingerprint_is_unchanged() {
    use crate::ui::state::State;
    use crate::ui::view::button;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree = ViewAdapter::build(button("First").on_click_capture(&state, move || {
        first_for_handler.set(first_for_handler.get() + 1);
    }));
    let pos = Point::new(4.0, 4.0);

    state.set(2);
    ViewAdapter::reconcile(
        &mut tree,
        button("Second").on_click_capture(&state, move || {
            second_for_handler.set(second_for_handler.get() + 1);
        }),
    );

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn button_dsl_window_capture_preserves_handler_when_fingerprint_is_unchanged() {
    use crate::ui::view::button;

    let window_id = WindowId::new(7);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree =
        ViewAdapter::build(button("First").on_click_window_capture(window_id, move || {
            first_for_handler.set(first_for_handler.get() + 1);
        }));
    let pos = Point::new(4.0, 4.0);

    ViewAdapter::reconcile(
        &mut tree,
        button("Second").on_click_window_capture(window_id, move || {
            second_for_handler.set(second_for_handler.get() + 1);
        }),
    );

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn input_dsl_state_capture_reregisters_handler_when_fingerprint_changes() {
    use crate::ui::state::State;
    use crate::ui::view::input;

    let first_state = State::new(1);
    let second_state = State::new(1);
    let first_value = Rc::new(RefCell::new(String::new()));
    let second_value = Rc::new(RefCell::new(String::new()));
    let first_for_handler = first_value.clone();
    let second_for_handler = second_value.clone();

    let mut tree = ViewAdapter::build(input().on_change_capture(&first_state, move |next| {
        *first_for_handler.borrow_mut() = next.to_string();
    }));
    let root = tree.root_id().unwrap();
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 120.0, 32.0));

    ViewAdapter::reconcile(
        &mut tree,
        input().on_change_capture(&second_state, move |next| {
            *second_for_handler.borrow_mut() = next.to_string();
        }),
    );

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::TextInput {
        text: "A".to_string(),
    });

    assert_eq!(&*first_value.borrow(), "");
    assert_eq!(&*second_value.borrow(), "A");
}

#[test]
fn input_dsl_window_capture_reregisters_handler_when_fingerprint_changes() {
    use crate::ui::view::input;

    let first_window = WindowId::new(7);
    let second_window = WindowId::new(8);
    let first_value = Rc::new(RefCell::new(String::new()));
    let second_value = Rc::new(RefCell::new(String::new()));
    let first_for_handler = first_value.clone();
    let second_for_handler = second_value.clone();

    let mut tree =
        ViewAdapter::build(input().on_change_window_capture(first_window, move |next| {
            *first_for_handler.borrow_mut() = next.to_string();
        }));
    let root = tree.root_id().unwrap();
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 120.0, 32.0));

    ViewAdapter::reconcile(
        &mut tree,
        input().on_change_window_capture(second_window, move |next| {
            *second_for_handler.borrow_mut() = next.to_string();
        }),
    );

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::TextInput {
        text: "A".to_string(),
    });

    assert_eq!(&*first_value.borrow(), "");
    assert_eq!(&*second_value.borrow(), "A");
}

#[test]
fn view_node_state_capture_preserves_handler_when_fingerprint_is_unchanged() {
    use crate::ui::state::State;
    use crate::ui::view::label;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree = ViewAdapter::build(label("First").on_semantic_capture(
        SemanticKind::Change,
        &state,
        move |_| {
            first_for_handler.set(first_for_handler.get() + 1);
        },
    ));
    let root_id = tree.root_id().unwrap();

    ViewAdapter::reconcile(
        &mut tree,
        label("Second").on_semantic_capture(SemanticKind::Change, &state, move |_| {
            second_for_handler.set(second_for_handler.get() + 1);
        }),
    );

    let _ = tree.dispatch_semantic(SemanticEvent::change(root_id, "next"));

    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn semantic_handler_macro_preserves_handler_when_fingerprint_is_unchanged() {
    use crate::ui::state::State;
    use crate::ui::view::label;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();
    let mut first = label("First").build();
    first.handlers = vec![crate::semantic_handler!(
        SemanticKind::Change,
        state[state],
        |_event| {
            first_for_handler.set(first_for_handler.get() + 1);
        }
    )];
    let mut tree = ViewAdapter::build_nodes(first);
    let root_id = tree.root_id().unwrap();

    let mut second = label("Second").build();
    second.handlers = vec![crate::semantic_handler!(
        SemanticKind::Change,
        state[state],
        |_event| {
            second_for_handler.set(second_for_handler.get() + 1);
        }
    )];
    ViewAdapter::reconcile_nodes(&mut tree, second);

    let _ = tree.dispatch_semantic(SemanticEvent::change(root_id, "next"));

    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn view_node_state_capture_reregisters_handler_when_fingerprint_changes() {
    use crate::ui::state::State;
    use crate::ui::view::label;

    let first_state = State::new(1);
    let second_state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree = ViewAdapter::build(label("First").on_semantic_capture(
        SemanticKind::Change,
        &first_state,
        move |_| {
            first_for_handler.set(first_for_handler.get() + 1);
        },
    ));
    let root_id = tree.root_id().unwrap();

    ViewAdapter::reconcile(
        &mut tree,
        label("Second").on_semantic_capture(SemanticKind::Change, &second_state, move |_| {
            second_for_handler.set(second_for_handler.get() + 1);
        }),
    );

    let _ = tree.dispatch_semantic(SemanticEvent::change(root_id, "next"));

    assert_eq!(first_hits.get(), 0);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn view_node_window_capture_reregisters_handler_when_fingerprint_changes() {
    use crate::ui::view::label;

    let first_window = WindowId::new(7);
    let second_window = WindowId::new(8);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree = ViewAdapter::build(label("First").on_semantic_window_capture(
        SemanticKind::Change,
        first_window,
        move |_| {
            first_for_handler.set(first_for_handler.get() + 1);
        },
    ));
    let root_id = tree.root_id().unwrap();

    ViewAdapter::reconcile(
        &mut tree,
        label("Second").on_semantic_window_capture(
            SemanticKind::Change,
            second_window,
            move |_| {
                second_for_handler.set(second_for_handler.get() + 1);
            },
        ),
    );

    let _ = tree.dispatch_semantic(SemanticEvent::change(root_id, "next"));

    assert_eq!(first_hits.get(), 0);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn test_state_auto_reconcile_invalidation() {
    use crate::ui::state::State;
    let state = State::new(42);
    let reconcile_called = Arc::new(AtomicBool::new(false));
    let reconcile_called_clone = reconcile_called.clone();
    state.set_reconcile_invalidation_fn(move || {
        reconcile_called_clone.store(true, Ordering::SeqCst)
    });
    assert!(!reconcile_called.load(Ordering::SeqCst));
    state.set(100);
    assert!(reconcile_called.load(Ordering::SeqCst));
}

#[test]
fn captured_state_set_requests_paint_without_reconcile_for_dynamic_label() {
    use crate::ui::state::State;
    use crate::ui::view::dynamic_label;

    let state = State::new(1);
    let mut tree = ViewAdapter::build({
        let state = state.clone();
        dynamic_label(move || format!("value-{}", state.get()))
    });
    let root_id = tree.root_id().expect("root should exist");
    tree.get_mut(root_id)
        .expect("root should be present")
        .set_frame(Rect::new(0.0, 0.0, 80.0, 24.0));
    tree.layout();
    tree.reset_invalidation();

    assert!(!tree.take_reconcile_requested());
    assert!(!tree.has_render_work());

    state.set(2);

    // DynamicLabel 仅绑 Paint：timer/counter 文本更新不得整树 reconcile+layout
    assert!(
        !tree.take_reconcile_requested(),
        "DynamicLabel state update must not request reconcile"
    );
    assert!(tree.has_render_work());
}

/// View 构建期 `State::get()` 的孤儿绑定必须 reconcile。
/// 仅 Paint 会导致选页等结构依赖全帧重绘却不重建树（demo 导航卡顿且内容不切换）。
/// 高频文本更新应走 DynamicLabel（见上例），不要在 build 期 `get()`。
#[test]
fn orphan_state_get_during_build_binds_reconcile() {
    use crate::ui::state::State;
    use crate::ui::view::label;

    let external = State::new(0.0f32);
    let root = ViewAdapter::capture_root({
        let external = external.clone();
        move || {
            let t = external.get(); // 须在 capture 内调用才会登记 orphan
            label(format!("t={t:.2}"))
        }
    });
    let mut tree = ViewAdapter::build_nodes(root);
    let root_id = tree.root_id().expect("root");
    tree.get_mut(root_id)
        .expect("root mut")
        .set_frame(Rect::new(0.0, 0.0, 100.0, 24.0));
    tree.layout();
    tree.reset_invalidation();
    assert!(!tree.take_reconcile_requested());

    external.set(1.0);
    assert!(
        tree.take_reconcile_requested(),
        "orphan State::get bind must request reconcile"
    );
}

#[test]
fn button_on_click_is_registered_as_semantic_handler() {
    use crate::ui::view::button;

    let clicks = Rc::new(Cell::new(0));
    let clicks_for_handler = clicks.clone();
    let mut tree = ViewAdapter::build(button("OK").on_click_fn(move || {
        clicks_for_handler.set(clicks_for_handler.get() + 1);
    }));

    let pos = Point::new(2.0, 2.0);
    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(clicks.get(), 1);
}

#[test]
fn input_on_change_is_registered_as_semantic_handler() {
    use crate::ui::view::input;

    let value = Rc::new(RefCell::new(String::new()));
    let value_for_handler = value.clone();
    let mut tree = ViewAdapter::build(input().on_change(move |next| {
        *value_for_handler.borrow_mut() = next.to_string();
    }));

    let root = tree.root_id().expect("input root should exist");
    tree.get_mut(root)
        .expect("input root should be present")
        .set_frame(Rect::new(0.0, 0.0, 120.0, 32.0));

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::TextInput {
        text: "A".to_string(),
    });

    assert_eq!(&*value.borrow(), "A");
}

#[test]
fn static_display_widgets_are_picture_eligible() {
    use crate::draw::scene::PicturePolicy;
    use crate::ui::widgets::{
        Alert, Avatar, Badge, BarChart, Content, Descriptions, Divider, Empty, Footer, Grid,
        Header, Icon, Layout, LineChart, List, PieChart, QRCode, ResultType, ResultView, Sider,
        Skeleton, Space, Tag, Timeline, Watermark,
    };

    let widgets: Vec<Box<dyn WidgetComponent>> = vec![
        Box::new(Space::new()),
        Box::new(Container::new()),
        Box::new(Grid::new()),
        Box::new(Divider::new()),
        Box::new(Icon::new("search")),
        Box::new(Avatar::new("A")),
        Box::new(Badge::new().count(8)),
        Box::new(Layout::new()),
        Box::new(Header::new(48.0)),
        Box::new(Sider::new(200.0)),
        Box::new(Content::new()),
        Box::new(Footer::new(40.0)),
        Box::new(Empty::new()),
        Box::new(Tag::new("stable")),
        Box::new(Descriptions::new()),
        Box::new(ResultView::new(ResultType::Info)),
        Box::new(Alert::new("stable")),
        Box::new(Timeline::new()),
        Box::new(Skeleton::new()),
        Box::new(List::new()),
        Box::new(BarChart::new()),
        Box::new(LineChart::new()),
        Box::new(PieChart::new()),
        Box::new(QRCode::new("stable")),
        Box::new(Watermark::new("stable")),
    ];

    for widget in widgets {
        assert_eq!(widget.picture_policy(), PicturePolicy::Eligible);
    }
}

#[test]
fn layout_bootstraps_after_invalidation_cleared_with_nonzero_measure_children() {
    use crate::ui::view::{column, label, row};

    // 与 demo shell 同构：水平 row = 侧栏 + 内容
    let root = row([
        column([label("侧栏项")]).width(120.0).flex_grow(0.0),
        column([label("内容"), label("页脚")]).flex_grow(1.0),
    ])
    .flex_grow(1.0);
    let mut tree = ViewAdapter::build_nodes(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
    }
    // 模拟 bind_invalidation / 帧末 reset：清空队列后子树仍为 zero frame
    tree.reset_invalidation();
    assert!(tree.layout_traverse().is_empty());

    tree.layout();

    let root_id = tree.root_id().expect("root");
    let main_children = tree.get(root_id).unwrap().children().to_vec();
    assert_eq!(main_children.len(), 2);
    let sidebar = tree.get(main_children[0]).unwrap().frame();
    let content = tree.get(main_children[1]).unwrap().frame();
    assert!(
        sidebar.w > 0.0 && sidebar.h > 0.0,
        "sidebar still zero: {sidebar:?}"
    );
    assert!(
        content.w > 0.0 && content.h > 0.0,
        "content still zero: {content:?}"
    );
    assert!(
        content.x >= sidebar.w - 0.5,
        "content should sit right of sidebar: sidebar={sidebar:?} content={content:?}"
    );

    let sidebar_container = tree
        .get(main_children[0])
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Container>()
        .expect("sidebar column is Container");
    assert_eq!(
        sidebar_container.style.flex_grow, 0.0,
        "explicit flex_grow(0) must survive Style::apply"
    );
}
