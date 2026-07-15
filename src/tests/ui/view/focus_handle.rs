use crate::tests::common::*;
use crate::ui::view::{input, EventExt, ViewAdapter};
use crate::ui::{FocusHandle, FocusHandleError};

#[test]
fn focus_handle_routes_focus_and_blur_through_app_state() {
    let handle = FocusHandle::new();
    let mut tree = ViewAdapter::build(input().focus_handle(&handle));
    let app_state = AppState::new();
    tree.set_app_state(app_state);
    tree.layout();
    let root = tree.root_id().expect("focus target should exist");

    assert!(handle.is_bound());
    assert_eq!(handle.focus(), Ok(()));
    assert!(tree.has_app_state_focus_requests());
    assert!(tree.drain_app_state_focus_requests());
    assert_eq!(tree.managers().focus.focused_component(), Some(root));

    assert_eq!(handle.blur(), Ok(()));
    assert!(tree.drain_app_state_focus_requests());
    assert_eq!(tree.managers().focus.focused_component(), None);
}

#[test]
fn unbound_focus_handle_returns_typed_error() {
    let handle = FocusHandle::new();

    assert!(!handle.is_bound());
    assert_eq!(handle.focus(), Err(FocusHandleError::Unbound));
    assert_eq!(handle.blur(), Err(FocusHandleError::Unbound));
}
