use std::cell::Cell;
use std::rc::Rc;

use crate::native::traits::input::{KeyCode, KeyMod, MouseButton};
use crate::ui::view::{button, label, EventExt, ViewAdapter};
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
