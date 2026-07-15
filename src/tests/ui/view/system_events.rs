use std::cell::Cell;
use std::rc::Rc;

use crate::native::traits::input::{KeyCode, KeyMod, MouseButton};
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::{button, column, label, EventExt, ViewAdapter};
use crate::ui::{EventResult, SystemEvent};

#[test]
fn view_raw_event_handler_participates_in_normal_dispatch() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut tree = ViewAdapter::build(label("target").on_event(move |event| {
        if matches!(
            event,
            SystemEvent::KeyDown {
                key: KeyCode::Enter,
                ..
            }
        ) {
            observed.set(observed.get() + 1);
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }));
    let root = tree.root_id().expect("root");

    assert_eq!(
        tree.dispatch_to(
            root,
            &SystemEvent::KeyDown {
                key: KeyCode::Enter,
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn view_raw_event_handler_falls_through_to_component_behavior() {
    let mut tree = ViewAdapter::build(button("target").on_event(|_| EventResult::NotHandled));
    let root = tree.root_id().expect("root");

    assert_eq!(
        tree.dispatch_to(
            root,
            &SystemEvent::PointerDown {
                pos: crate::core::Point::new(1.0, 1.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );
}

#[test]
fn view_raw_event_handlers_are_replaced_during_reconcile() {
    let first_calls = Rc::new(Cell::new(0));
    let first_observed = Rc::clone(&first_calls);
    let mut tree = ViewAdapter::build(label("first").on_event(move |_| {
        first_observed.set(first_observed.get() + 1);
        EventResult::Handled
    }));
    let root = tree.root_id().expect("root");
    assert_eq!(
        tree.dispatch_to(root, &SystemEvent::FocusIn),
        EventResult::Handled
    );

    let second_calls = Rc::new(Cell::new(0));
    let second_observed = Rc::clone(&second_calls);
    ViewAdapter::reconcile(
        &mut tree,
        label("second").on_event(move |_| {
            second_observed.set(second_observed.get() + 1);
            EventResult::Handled
        }),
    );

    assert_eq!(
        tree.dispatch_to(root, &SystemEvent::FocusOut),
        EventResult::Handled
    );
    assert_eq!(first_calls.get(), 1);
    assert_eq!(second_calls.get(), 1);
}

#[test]
fn categorized_view_handlers_only_receive_their_event_family() {
    let pointer_calls = Rc::new(Cell::new(0));
    let key_calls = Rc::new(Cell::new(0));
    let focus_calls = Rc::new(Cell::new(0));
    let scroll_calls = Rc::new(Cell::new(0));
    let mut tree = ViewAdapter::build(
        label("target")
            .on_pointer({
                let calls = Rc::clone(&pointer_calls);
                move |_| {
                    calls.set(calls.get() + 1);
                    EventResult::NotHandled
                }
            })
            .on_key({
                let calls = Rc::clone(&key_calls);
                move |_| {
                    calls.set(calls.get() + 1);
                    EventResult::NotHandled
                }
            })
            .on_focus({
                let calls = Rc::clone(&focus_calls);
                move |_| {
                    calls.set(calls.get() + 1);
                    EventResult::NotHandled
                }
            })
            .on_scroll({
                let calls = Rc::clone(&scroll_calls);
                move |_| {
                    calls.set(calls.get() + 1);
                    EventResult::NotHandled
                }
            }),
    );
    let root = tree.root_id().expect("root");

    let events = [
        SystemEvent::PointerMove {
            pos: crate::core::Point::new(1.0, 1.0),
            mods: KeyMod::NONE,
        },
        SystemEvent::KeyUp {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        },
        SystemEvent::FocusIn,
        SystemEvent::Wheel {
            pos: crate::core::Point::new(1.0, 1.0),
            delta: crate::core::Point::new(0.0, 8.0),
        },
    ];
    for event in &events {
        let _ = tree.dispatch_to(root, event);
    }

    assert_eq!(pointer_calls.get(), 1);
    assert_eq!(key_calls.get(), 1);
    assert_eq!(focus_calls.get(), 1);
    assert_eq!(scroll_calls.get(), 1);
    assert!(tree
        .get(root)
        .is_some_and(|node| node.wants_continuous_pointer_move()));
}

#[test]
fn capture_handler_intercepts_focused_child_key_event() {
    let capture_calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&capture_calls);
    let mut tree = ViewAdapter::build(
        column((label("child")
            .on_key(|_| EventResult::NotHandled)
            .focusable(true),))
        .on_key_capture(move |_| {
            observed.set(observed.get() + 1);
            EventResult::Handled
        }),
    );
    let root = tree.root_id().expect("root");
    let child = tree
        .get(root)
        .and_then(|node| node.children().first().copied())
        .expect("child");
    tree.set_focus(Some(child));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(capture_calls.get(), 1);
}

#[test]
fn raw_handler_node_enters_and_leaves_tab_order_during_reconcile() {
    let focus_events = Rc::new(Cell::new(0));
    let observed = Rc::clone(&focus_events);
    let mut tree = ViewAdapter::build(
        label("focusable")
            .on_focus(move |_| {
                observed.set(observed.get() + 1);
                EventResult::Handled
            })
            .focusable(true),
    );
    let root = tree.root_id().expect("root");
    assert_eq!(tree.collect_focusable(), vec![root]);
    tree.set_focus(Some(root));
    assert_eq!(focus_events.get(), 1);

    ViewAdapter::reconcile(
        &mut tree,
        label("not-focusable")
            .on_focus(|_| EventResult::Handled)
            .focusable(false),
    );

    assert!(tree.collect_focusable().is_empty());
    assert_eq!(tree.managers().focus.focused_component(), Some(root));
}
