use std::cell::Cell;
use std::rc::Rc;

use crate::ui::semantic_action::SemanticAction;
use crate::ui::view::{button, label, AccessibilityExt, ViewAdapter};
use crate::ui::{AccessibilityRole, AccessibilityState, ComponentHandle};

#[test]
fn view_accessibility_patch_overrides_explicit_fields() {
    let tree = ViewAdapter::build(
        label("base")
            .role(AccessibilityRole::Button)
            .accessible_name("Submit")
            .accessibility_state(AccessibilityState::disabled(true))
            .aria("aria-description", "Saves the document"),
    );
    let snapshot = tree.semantic_snapshot_body();
    let accessibility = &snapshot.nodes[0].accessibility;

    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.name.as_deref(), Some("Submit"));
    assert!(accessibility.state.disabled);
    assert!(accessibility
        .aria_attributes()
        .iter()
        .any(|attribute| attribute.name == "aria-description"
            && attribute.value == "Saves the document"));
}

#[test]
fn partial_view_override_preserves_component_name_and_runtime_state() {
    let tree = ViewAdapter::build(button("Save").disabled(true).role(AccessibilityRole::Alert));
    let accessibility = &tree.semantic_snapshot_body().nodes[0].accessibility;

    assert_eq!(accessibility.role, AccessibilityRole::Alert);
    assert_eq!(accessibility.name.as_deref(), Some("Save"));
    assert!(accessibility.state.disabled);
}

#[test]
fn accessibility_override_reconciles_and_component_handle_reads_it() {
    let tree = ViewAdapter::build(
        label("before")
            .role(AccessibilityRole::Button)
            .accessible_name("Before"),
    );
    let tree = Rc::new(std::cell::RefCell::new(tree));
    let root = tree.borrow().root_id().expect("root");
    let handle = ComponentHandle::new(root, &tree);
    assert_eq!(
        handle.accessibility().map(|snapshot| snapshot.role),
        Some(AccessibilityRole::Button)
    );

    ViewAdapter::reconcile(&mut tree.borrow_mut(), label("after"));

    assert_eq!(
        handle.accessibility().map(|snapshot| snapshot.role),
        Some(AccessibilityRole::Text)
    );
    assert_eq!(
        handle.accessibility().and_then(|snapshot| snapshot.name),
        Some("after".into())
    );
}

#[test]
fn custom_button_role_falls_back_to_semantic_invoke_handler() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut tree = ViewAdapter::build(
        label("Launch")
            .on_click_fn(move || observed.set(observed.get() + 1))
            .role(AccessibilityRole::Button),
    );
    let root = tree.root_id().expect("root");

    tree.perform_semantic_action(root, &SemanticAction::Invoke)
        .expect("custom button invoke");

    assert_eq!(calls.get(), 1);
}
