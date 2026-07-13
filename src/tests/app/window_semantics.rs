use crate::app::window_semantics::WindowSemanticState;
use crate::core::{Rect, WindowId};
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::combinators::button;
use crate::ui::view::ViewAdapter;

#[test]
fn semantic_revision_changes_only_for_observable_state_and_close() {
    let mut tree = ViewAdapter::build_nodes(button("Track me").into());
    let root = tree.root_id().expect("button root should exist");
    tree.get_mut(root)
        .expect("button root should exist")
        .set_frame(Rect::new(0.0, 0.0, 160.0, 40.0));
    tree.layout();

    let mut state = WindowSemanticState::new(WindowId::new(7));
    assert!(state.snapshot().is_none(), "tracking must be opt-in");
    assert!(state.enable(&tree));
    let initial = state.snapshot().expect("tracking should expose a snapshot");
    assert_eq!(initial.window_id, WindowId::new(7));
    assert_eq!(initial.generation, 1);
    assert_eq!(initial.revision, 1);
    assert_eq!(initial.presented_revision, 0);
    assert!(!initial.closed);
    assert_eq!(initial.nodes.len(), 1);

    assert!(!state.refresh(&tree));
    assert_eq!(state.snapshot().unwrap().revision, 1);

    tree.set_focus(Some(root));
    assert!(state.refresh(&tree));
    let focused = state.snapshot().unwrap();
    assert_eq!(focused.revision, 2);
    assert!(focused.nodes[0].focused);
    assert!(!state.refresh(&tree));
    assert_eq!(state.snapshot().unwrap().revision, 2);

    assert!(state.mark_presented());
    assert_eq!(state.snapshot().unwrap().presented_revision, 2);
    assert!(!state.mark_presented());

    assert!(state.close());
    let closed = state.snapshot().unwrap();
    assert_eq!(closed.revision, 3);
    assert_eq!(closed.presented_revision, 2);
    assert!(closed.closed);
    assert!(closed.nodes.is_empty());
    assert!(!state.close());
}
