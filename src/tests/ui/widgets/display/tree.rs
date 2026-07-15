use crate::tests::common::*;
use crate::ui::widgets::display::tree::*;

fn key_event(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    }
}

#[test]
fn tree_keyboard_navigation_expands_selects_checks_and_collapses() {
    let nodes = vec![
        TreeNode::new("Root", "root").children(vec![
            TreeNode::new("Disabled", "disabled").disabled(true),
            TreeNode::new("Child", "child").checkable(true),
        ]),
        TreeNode::new("Other", "other"),
    ];
    let mut tree = Tree::new(nodes);

    assert_eq!(WidgetComponent::tab_index(&tree), 1);
    assert_eq!(tree.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(
        tree.on_event(&key_event(KeyCode::Down)),
        EventResult::Handled
    );
    assert_eq!(tree.selected_key(), "root");

    tree.on_event(&key_event(KeyCode::Right));
    tree.on_event(&key_event(KeyCode::Right));
    assert_eq!(tree.selected_key(), "child");

    tree.on_event(&key_event(KeyCode::Space));
    assert!(matches!(
        tree.snapshot_fields(),
        SnapshotFields::Tree {
            nodes,
            selected_key,
            expanded_keys,
            ..
        } if nodes[0].children[1].checked
            && selected_key == "child"
            && expanded_keys == ["root"]
    ));

    tree.on_event(&key_event(KeyCode::Left));
    assert_eq!(tree.selected_key(), "root");
    tree.on_event(&key_event(KeyCode::Left));
    assert!(matches!(
        tree.snapshot_fields(),
        SnapshotFields::Tree { expanded_keys, .. } if expanded_keys.is_empty()
    ));
}

#[test]
fn tree_keyboard_navigation_skips_disabled_rows_and_reveals_selection() {
    let nodes = vec![
        TreeNode::new("First", "first"),
        TreeNode::new("Disabled", "disabled").disabled(true),
        TreeNode::new("Third", "third"),
        TreeNode::new("Fourth", "fourth"),
    ];
    let mut tree = Tree::new(nodes);
    tree.last_frame.set(Some(Rect::new(0.0, 0.0, 200.0, 56.0)));

    tree.on_event(&key_event(KeyCode::Down));
    tree.on_event(&key_event(KeyCode::Down));
    assert_eq!(tree.selected_key(), "third");
    assert!(tree.body_scroll.scroll_offset() > 0.0);
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&tree),
        Some((0.0, 28.0))
    );
}

#[test]
fn tree_selection_emits_structured_change_event() {
    let mut tree = Tree::new(vec![TreeNode::new("First", "first")]);
    let event = key_event(KeyCode::Down);
    tree.on_event(&event);

    let semantic = tree
        .semantic_event(ComponentId::new(7), &event)
        .expect("keyboard selection should emit change");
    assert_eq!(semantic.text_payload(), Some("first"));
}
