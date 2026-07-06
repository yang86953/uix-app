use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn handler_table_bubbles_until_stopped() {
        let child = 2;
        let parent = 1;
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
    fn handler_table_once_removes_after_first_dispatch() {
        let id = 1;
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
        let id = 1;
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

    #[derive(Debug, PartialEq)]
    struct BusinessPayload {
        value: u32,
    }

    #[test]
    fn register_semantic_macro_matches_custom_event_kind() {
        let event = SemanticEvent::custom(1, BusinessPayload { value: 7 });

        assert_eq!(event.kind, register_semantic!(BusinessPayload));
    }

    #[test]
    fn handler_table_dispatches_typed_custom_payload() {
        let id = 1;
        let value = Rc::new(Cell::new(0));
        let value_for_handler = value.clone();
        let mut table = HandlerTable::new();
        table.on_custom::<BusinessPayload>(id, move |payload| {
            value_for_handler.set(payload.value);
        });

        let mut event = SemanticEvent::custom(1, BusinessPayload { value: 42 });
        let result = table.dispatch_path(&[id], &mut event);

        assert_eq!(result, EventResult::Handled);
        assert_eq!(value.get(), 42);
    }
