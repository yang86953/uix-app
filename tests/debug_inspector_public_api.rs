use uix::core::{Rect, WidgetId};
use uix::draw::scene::{HoverInspectorNode, HoverInspectorSnapshot};

fn node(id: usize, type_name: &'static str, stable_id: &str, frame: Rect) -> HoverInspectorNode {
    HoverInspectorNode {
        node_id: WidgetId::new(id),
        type_name,
        stable_id: stable_id.to_string(),
        frame,
        z_index: id as i32,
        child_count: usize::from(id == 0),
        visible: true,
        dirty: id == 1,
        hovered: id == 1,
        pressed: false,
        focused: id == 1,
        disabled: false,
        attached: true,
        mounted: true,
        active: true,
        pending_removal: false,
    }
}

#[test]
fn inspector_snapshot_formats_identity_state_lifecycle_and_geometry() {
    let snapshot = HoverInspectorSnapshot {
        nodes: vec![
            node(
                0,
                "uix::ui::containers::RootContainer",
                "automation:root",
                Rect::new(0.0, 0.0, 800.0, 600.0),
            ),
            node(
                1,
                "uix::ui::general::Button",
                "automation:save",
                Rect::new(20.0, 30.0, 96.0, 32.0),
            ),
        ],
    };

    assert_eq!(
        snapshot.leaf().map(|node| node.node_id),
        Some(WidgetId::new(1))
    );
    let lines = snapshot.inspector_lines(8);
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("depth=2"));
    assert!(lines[1].contains("RootContainer automation:root"));
    assert!(lines[1].contains("[----V-|AMX-]"));
    assert!(lines[2].contains("Button automation:save"));
    assert!(lines[2].contains("[H-F-V*|AMX-]"));
    assert!(lines[2].contains("(20,30 96x32) z1 c0"));
}

#[test]
fn inspector_snapshot_bounds_deep_paths_without_losing_leaf_index() {
    let snapshot = HoverInspectorSnapshot {
        nodes: vec![
            node(0, "Root", "0:0", Rect::default()),
            node(1, "Leaf", "1:0", Rect::default()),
        ],
    };

    let lines = snapshot.inspector_lines(1);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1], "... 1 ancestors omitted");
    assert!(lines[2].starts_with("01 Leaf 1:0"));
}
