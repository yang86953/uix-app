use crate::tests::common::*;
use crate::ui::widgets::display::tree::TreeNode;
use crate::ui::widgets::input::tree_select::*;
use crate::ui::AccessibilityRole;

fn large_tree_select() -> TreeSelect {
    let nodes: Vec<TreeNode> = (0..80)
        .map(|i| TreeNode::new(&format!("Node {i}"), &format!("n-{i}")))
        .collect();
    TreeSelect::new().nodes(nodes)
}

#[test]
fn tree_select_dropdown_scroll_range_limits_visible_rows() {
    let tree_select = large_tree_select();
    let row_count = tree_select.flatten_nodes().len();
    let viewport_h = tree_select.dropdown_viewport_height(row_count);
    let (start, end) = tree_select
        .dropdown_scroll
        .scroll_range(row_count, 28.0, viewport_h);
    assert_eq!(start, 0);
    assert!(
        end - start < 80,
        "virtual scroll should expose a small window"
    );
}

#[test]
fn tree_select_dropdown_wheel_records_composite_delta() {
    let mut tree_select = large_tree_select();
    tree_select.open();

    assert_eq!(
        EventHandler::on_event(
            &mut tree_select,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 50.0),
                delta: Point::new(0.0, -1.0),
            },
        ),
        EventResult::Handled
    );
    assert!(tree_select.dropdown_scroll.scroll_offset() > 0.0);
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&tree_select),
        Some((0.0, 40.0))
    );
}

#[test]
fn tree_select_dropdown_row_at_y_accounts_for_scroll_offset() {
    let mut tree_select = large_tree_select();
    tree_select.open();
    tree_select.dropdown_scroll.set_scroll_offset(28.0 * 5.0);

    assert_eq!(tree_select.dropdown_row_at_y(33.0), Some(5));
    assert_eq!(tree_select.dropdown_row_at_y(61.0), Some(6));
}

#[test]
fn tree_select_keyboard_skips_disabled_nodes_and_commits_with_enter() {
    let mut tree_select = TreeSelect::new().nodes(vec![
        TreeNode::new("Disabled", "disabled").disabled(true),
        TreeNode::new("Alpha", "alpha"),
        TreeNode::new("Beta", "beta"),
    ]);

    assert_eq!(
        tree_select.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let _ = tree_select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    let _ = tree_select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });

    assert_eq!(tree_select.value(), "Beta");
    assert_eq!(tree_select.value_key(), "beta");
    assert!(!tree_select.is_open());
    let accessibility = tree_select.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Combobox);
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Beta"));
    assert_eq!(accessibility.state.expanded, Some(false));
}

#[test]
fn tree_select_disabled_pointer_row_stays_open_and_unselected() {
    let mut tree_select = TreeSelect::new().nodes(vec![
        TreeNode::new("Disabled", "disabled").disabled(true),
        TreeNode::new("Enabled", "enabled"),
    ]);
    tree_select.open();

    assert_eq!(
        tree_select.on_event(&SystemEvent::PointerDown {
            pos: Point::new(8.0, 33.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(tree_select.is_open());
    assert!(tree_select.value_key().is_empty());
}

#[test]
fn tree_select_keyboard_reveals_highlight_in_long_dropdown() {
    let mut tree_select = large_tree_select();
    tree_select.open();
    for _ in 0..20 {
        let _ = tree_select.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        });
    }

    assert!(tree_select.dropdown_scroll.scroll_offset() > 0.0);
    assert!(EventHandler::scroll_delta_for_dirty(&tree_select).is_some());
}

#[test]
fn tree_select_focus_out_closes_and_snapshot_preserves_current_value() {
    let mut tree_select = TreeSelect::new().nodes(vec![TreeNode::new("Alpha", "alpha")]);
    tree_select.open();
    let _ = tree_select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    tree_select.open();
    let _ = tree_select.on_event(&SystemEvent::FocusOut);

    assert!(!tree_select.is_open());
    assert!(matches!(
        tree_select.snapshot_fields(),
        SnapshotFields::TreeSelect {
            value,
            value_key,
            open: false,
            ..
        } if value == "Alpha" && value_key == "alpha"
    ));
}
