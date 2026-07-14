use crate::ui::state::{Computed, State};
use crate::ui::view::{column, label, ViewAdapter, ViewNode};
use crate::ui::widgets::Label;

fn conditional_root(computed: &Computed<bool>) -> ViewNode {
    column((
        label("always"),
        computed.map_opt(|visible| visible.then(|| label("conditional"))),
    ))
}

#[test]
fn computed_map_and_map_text_build_documented_views() {
    let source = State::new(7);
    let computed = Computed::new({
        let source = source.clone();
        move || source.get() * 2
    });

    let mapped = computed.map(|value| label(format!("value={value}")));
    let mapped_label = mapped
        .widget
        .as_any()
        .downcast_ref::<Label>()
        .expect("computed map should build a Label");
    assert_eq!(mapped_label.text(), "value=14");

    let text = computed.map_text(|value| format!("text={value}"));
    assert!(text
        .widget
        .as_any()
        .downcast_ref::<crate::ui::view::combinators::DynamicLabel>()
        .is_some());
}

#[test]
fn cached_computed_map_opt_reconciles_when_its_source_changes() {
    let visible = State::new(true);
    let computed = Computed::new({
        let visible = visible.clone();
        move || visible.get()
    });

    let root = ViewAdapter::capture_root(|| conditional_root(&computed));
    let mut tree = ViewAdapter::build_nodes(root);
    assert_eq!(tree.find_all_by_type::<Label>().len(), 2);
    tree.reset_invalidation();

    visible.set(false);
    assert!(tree.take_reconcile_requested());

    let next = ViewAdapter::capture_root(|| conditional_root(&computed));
    ViewAdapter::reconcile_nodes(&mut tree, next);
    assert_eq!(tree.find_all_by_type::<Label>().len(), 1);
}
