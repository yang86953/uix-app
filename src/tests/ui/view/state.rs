use crate::ui::state::State;
use crate::ui::view::{column, label, ViewAdapter, ViewNode};
use crate::ui::widgets::Label;

fn conditional_root(state: &State<bool>) -> ViewNode {
    column((
        label("always"),
        state.map_opt(|visible| visible.then(|| label("conditional"))),
    ))
}

#[test]
fn state_map_builds_view_from_current_value() {
    let state = State::new(7);

    let mapped = state.map(|value| label(format!("value={value}")));
    let mapped_label = mapped
        .widget
        .as_any()
        .downcast_ref::<Label>()
        .expect("state map should build a Label");

    assert_eq!(mapped_label.text(), "value=7");
}

#[test]
fn state_map_opt_reconciles_when_source_changes() {
    let visible = State::new(true);

    let root = ViewAdapter::capture_root(|| conditional_root(&visible));
    let mut tree = ViewAdapter::build_nodes(root);
    assert_eq!(tree.find_all_by_type::<Label>().len(), 2);
    tree.reset_invalidation();

    visible.set(false);
    assert!(tree.take_reconcile_requested());

    let next = ViewAdapter::capture_root(|| conditional_root(&visible));
    ViewAdapter::reconcile_nodes(&mut tree, next);
    assert_eq!(tree.find_all_by_type::<Label>().len(), 1);
}
