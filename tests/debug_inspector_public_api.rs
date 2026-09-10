use std::time::Duration;

use uix_app::core::{Rect, WidgetId};
use uix_app::draw::debug::DebugFrameSnapshot;
use uix_app::draw::renderer::{InvalidationSource, RenderMetrics};
use uix_app::draw::scene::{HoverInspectorNode, HoverInspectorSnapshot};

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
                "uix_app::ui::containers::RootContainer",
                "automation:root",
                Rect::new(0.0, 0.0, 800.0, 600.0),
            ),
            node(
                1,
                "uix_app::ui::general::Button",
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

#[test]
fn frame_snapshot_separates_single_frame_facts_from_cumulative_calls() {
    let snapshot = DebugFrameSnapshot {
        frame_sequence: 42,
        correlation_id: Some(7),
        tree_version: 19,
        tree_version_delta: 2,
        frame_time: Duration::from_micros(12_500),
        layout_time: Duration::from_micros(500),
        render_time: Duration::from_millis(4),
        submit_time: Duration::from_millis(2),
        present_time: Duration::from_millis(6),
        dirty_full: false,
        dirty_rect_count: 3,
        dirty_area_ratio: 0.125,
        invalidation_count: 4,
        largest_invalidation_slot: Some(9),
        largest_invalidation_ratio: 0.08,
        animation_count: 1,
        reconcile_ran: true,
        invalidation_source: InvalidationSource::LayoutEvent,
    };
    let metrics = RenderMetrics {
        layout_calls: 5,
        paint_calls: 6,
        present_calls: 7,
        idle_frames: 8,
        last_invalidation: InvalidationSource::LayoutEvent,
    };

    let lines = snapshot.hud_lines(Some(&metrics));

    assert!(lines[0].contains("frame #42  corr 7  tree 19 (+2)"));
    assert!(lines.iter().any(|line| line.contains("total 12.50 ms")));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("damage partial 12.5%  rects 3"))
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("inv layout_event  count 4  max #9 8.0%"))
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("calls layout 5  paint 6  present 7  idle 8"))
    );
}
