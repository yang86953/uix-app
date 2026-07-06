use super::*;
use crate::app::main_thread_queue::MainThreadContext;
use crate::draw::NullEngine;
use crate::ui::view::combinators::label;
use crate::ui::view::ViewAdapter;
use crate::ui::view::ViewNode;
use crate::ui::widgets::container::Container;
use crate::ui::widgets::Label;
use crate::ui::{AppState, WidgetCore};

fn root_label_text(root: ViewNode) -> String {
    let tree = ViewAdapter::build_nodes(root);
    tree.root()
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap()
        .text()
        .to_string()
}

#[test]
fn window_session_builds_tree_engine_and_p0_state() {
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        320,
        240,
    );

    assert_eq!(session.loop_state(), WindowLoopState::Active);
    assert!(session.active_work().is_empty());
    assert!(session.window_visible());
    assert!(!session.view_factory().is_installed());

    let (tree, _engine) = session.tree_and_engine_mut();
    let root_id = tree.root_id().expect("session root should exist");
    let root = tree.get(root_id).expect("session root should be present");

    assert_eq!(root.frame().w, 320.0);
    assert_eq!(root.frame().h, 240.0);
    assert!(tree.has_render_work());
}

#[test]
fn window_session_injects_app_state_into_existing_mounted_tree() {
    let mut session =
        WindowSession::from_root(label("root"), Box::new(NullEngine::new()), 320, 240);
    let app_state = AppState::new();
    session.set_app_state(app_state.clone());

    let (tree, _engine) = session.tree_and_engine_mut();
    let root_id = tree.root_id().unwrap();

    assert_eq!(
        app_state.get_handle(root_id).unwrap().text().as_deref(),
        Some("root")
    );
}

#[test]
fn window_session_factory_builds_reconcile_root_when_no_pending_root() {
    let mut session = WindowSession::from_root_factory(
        || label("factory"),
        Box::new(NullEngine::new()),
        320,
        240,
    );

    assert!(session.view_factory().is_installed());
    session.request_reconcile();

    let root = session
        .take_pending_root()
        .or_else(|| session.view_factory().build())
        .unwrap();

    assert!(session.reconcile_pending());
    assert_eq!(root_label_text(root), "factory");
}

#[test]
fn window_session_pending_root_is_one_shot_and_overrides_factory() {
    let mut session = WindowSession::from_root_factory(
        || label("factory"),
        Box::new(NullEngine::new()),
        320,
        240,
    );

    {
        let parts = session.parts_mut();
        let mut context = MainThreadContext::new(parts.pending_root, parts.reconcile_pending);
        context.update_root(label("pending"));
    }

    let pending = session.take_pending_root().unwrap();
    assert_eq!(root_label_text(pending), "pending");
    assert!(session.take_pending_root().is_none());
    assert_eq!(
        root_label_text(session.view_factory().build().unwrap()),
        "factory"
    );
}
