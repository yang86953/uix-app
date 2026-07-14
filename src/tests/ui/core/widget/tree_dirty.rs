use std::sync::Arc;

use crate::ui::WidgetTree;

#[test]
fn reconcile_requester_reuses_cached_callback() {
    let tree = WidgetTree::new();

    let first = tree.reconcile_requester();
    let second = tree.reconcile_requester();

    assert!(Arc::ptr_eq(&first, &second));
    first();
    assert!(tree.take_reconcile_requested());
}
