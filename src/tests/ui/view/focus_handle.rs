use crate::tests::common::*;
use crate::ui::view::{column, input, label, EventExt, View, ViewAdapter};
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

#[test]
fn reconcile_rebinds_same_node_and_invalidates_replaced_handle() {
    let first = FocusHandle::new();
    let second = FocusHandle::new();
    let mut tree = ViewAdapter::build(input().focus_handle(&first));
    tree.set_app_state(AppState::new());
    tree.layout();
    let root = tree.root_id().expect("focus target should exist");

    ViewAdapter::reconcile(&mut tree, input().focus_handle(&second));

    assert_eq!(tree.root_id(), Some(root));
    assert_eq!(first.focus(), Err(FocusHandleError::Unbound));
    assert!(second.is_bound());
    assert_eq!(second.focus(), Ok(()));
    assert!(tree.drain_app_state_focus_requests());
    assert_eq!(tree.managers().focus.focused_component(), Some(root));
}

#[test]
fn removing_bound_view_invalidates_focus_handle() {
    let handle = FocusHandle::new();
    let mut tree = ViewAdapter::build(column((
        input().build().key("field").focus_handle(&handle),
        label("kept").key("label"),
    )));
    tree.set_app_state(AppState::new());
    tree.layout();

    ViewAdapter::reconcile(&mut tree, column((label("kept").key("label"),)));

    assert!(!handle.is_bound());
    assert_eq!(handle.focus(), Err(FocusHandleError::Unbound));
    assert!(!tree.has_app_state_focus_requests());
}

#[test]
fn focus_requests_are_isolated_to_bound_window_tree() {
    let app_state = AppState::new();
    let first = FocusHandle::new();
    let second = FocusHandle::new();
    let mut first_tree = ViewAdapter::build(input().focus_handle(&first));
    let mut second_tree = ViewAdapter::build(input().focus_handle(&second));
    first_tree.set_app_state(app_state.clone());
    second_tree.set_app_state(app_state);
    first_tree.layout();
    second_tree.layout();
    let first_root = first_tree.root_id().expect("first focus target");

    assert_eq!(first.focus(), Ok(()));
    assert!(!second_tree.has_app_state_focus_requests());
    assert!(!second_tree.drain_app_state_focus_requests());
    assert!(first_tree.drain_app_state_focus_requests());
    assert_eq!(
        first_tree.managers().focus.focused_component(),
        Some(first_root)
    );
    assert_eq!(second_tree.managers().focus.focused_component(), None);
}

#[test]
fn sharing_handle_across_windows_reports_ambiguous_target() {
    let app_state = AppState::new();
    let handle = FocusHandle::new();
    let mut first_tree = ViewAdapter::build(input().focus_handle(&handle));
    let mut second_tree = ViewAdapter::build(input().focus_handle(&handle));
    first_tree.set_app_state(app_state.clone());
    second_tree.set_app_state(app_state);
    first_tree.layout();
    second_tree.layout();

    assert_eq!(
        handle.focus(),
        Err(FocusHandleError::AmbiguousTarget { count: 2 })
    );
}

#[test]
fn background_focus_request_is_drained_on_owner_ui_thread() {
    let handle = FocusHandle::new();
    let mut tree = ViewAdapter::build(input().focus_handle(&handle));
    tree.set_app_state(AppState::new());
    tree.layout();
    let root = tree.root_id().expect("focus target should exist");
    let background_handle = handle.clone();

    let result = std::thread::spawn(move || background_handle.focus())
        .join()
        .expect("background focus request should not panic");

    assert_eq!(result, Ok(()));
    assert!(tree.drain_app_state_focus_requests());
    assert_eq!(tree.managers().focus.focused_component(), Some(root));
}

#[test]
fn unavailable_target_consumes_request_without_forcing_focus() {
    let handle = FocusHandle::new();
    let mut tree = ViewAdapter::build(input().focus_handle(&handle));
    tree.set_app_state(AppState::new());
    tree.layout();
    let root = tree.root_id().expect("focus target should exist");
    tree.set_visible(root, false);

    assert_eq!(handle.focus(), Ok(()));
    assert!(tree.drain_app_state_focus_requests());
    assert_eq!(tree.managers().focus.focused_component(), None);
    assert!(!tree.has_app_state_focus_requests());
}

#[cfg(feature = "test-harness")]
#[test]
fn test_app_settle_drains_public_focus_handle() {
    use crate::ui::test_harness::TestApp;

    let handle = FocusHandle::new();
    let build_handle = handle.clone();
    let mut app = TestApp::new((320.0, 200.0), move || input().focus_handle(&build_handle));
    let root = app.tree().root_id().expect("focus target should exist");

    assert_eq!(handle.focus(), Ok(()));
    app.settle().expect("focus request should settle");

    assert_eq!(app.tree().managers().focus.focused_component(), Some(root));
}
