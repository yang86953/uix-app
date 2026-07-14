use crate::tests::common::*;
use crate::ui::layout::{GridTrack, JustifyContent};
use crate::ui::widgets::containers::grid::*;
use crate::ui::widgets::Container;

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.01,
        "expected {expected}, got {actual}"
    );
}

fn responsive_child_frames(grid: Grid, frame: Rect, count: usize) -> Vec<Rect> {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(grid));
    let children: Vec<_> = (0..count)
        .map(|_| tree.add_child(root, Box::new(Container::new().size(10.0, 10.0))))
        .collect();
    tree.get_mut(root)
        .expect("responsive grid root")
        .set_frame(frame);
    tree.push_layout_invalidation(root);
    tree.layout();
    children
        .into_iter()
        .map(|id| tree.get(id).expect("responsive grid child").frame())
        .collect()
}

#[test]
fn measure_children_clamps_size_and_preserves_grid_metadata() {
    let mut tree = WidgetTree::new();
    let child = tree.set_root(Box::new(
        Container::new()
            .style(
                Style::container()
                    .with_grid_cell(2)
                    .with_grid_column_span(3)
                    .with_grid_row_span(4),
            )
            .size(140.0, 90.0),
    ));
    let grid = Grid::new().columns(vec![GridTrack::Auto]).size(100.0, 80.0);

    let measured = grid.measure_children(Rect::new(0.0, 0.0, 100.0, 80.0), &[child], &tree);

    assert_eq!(measured.len(), 1);
    assert_eq!(measured[0].id, child);
    assert_eq!(measured[0].measured_size, Size::new(100.0, 80.0));
    assert_eq!(measured[0].grid_cell, Some(2));
    assert_eq!(measured[0].grid_column_span, 3);
    assert_eq!(measured[0].grid_row_span, 4);
}

#[test]
fn breakpoints_reject_invalid_custom_thresholds() {
    assert_eq!(
        Breakpoints::new(f32::NAN, 768.0, 992.0, 1200.0, 1600.0),
        Err(BreakpointError::NonFinite)
    );
    assert_eq!(
        Breakpoints::new(-1.0, 768.0, 992.0, 1200.0, 1600.0),
        Err(BreakpointError::Negative)
    );
    assert_eq!(
        Breakpoints::new(576.0, 576.0, 992.0, 1200.0, 1600.0),
        Err(BreakpointError::NotStrictlyAscending)
    );

    let custom = Breakpoints::new(600.0, 800.0, 1000.0, 1200.0, 1600.0)
        .expect("strictly ascending breakpoints");
    assert_eq!(custom.xs(), 0.0);
    assert_eq!(custom.sm(), 600.0);
    assert_eq!(custom.md(), 800.0);
    assert_eq!(custom.lg(), 1000.0);
    assert_eq!(custom.xl(), 1200.0);
    assert_eq!(custom.xxl(), 1600.0);
}

#[test]
fn responsive_grid_switches_spans_at_logical_breakpoints() {
    let col = Col::new().span(24).sm(12).md(8).lg(6);
    let xs = responsive_child_frames(
        Grid::responsive()
            .cols(vec![col; 3])
            .justify(JustifyContent::Stretch),
        Rect::new(0.0, 0.0, 480.0, 300.0),
        3,
    );
    assert_close(xs[0].x, 0.0);
    assert_close(xs[1].x, 0.0);
    assert_close(xs[2].x, 0.0);
    assert_close(xs[0].w, 480.0);
    assert_close(xs[1].y, 100.0);
    assert_close(xs[2].y, 200.0);

    let md = responsive_child_frames(
        Grid::responsive()
            .cols(vec![col; 3])
            .justify(JustifyContent::Stretch),
        Rect::new(0.0, 0.0, 800.0, 300.0),
        3,
    );
    assert_close(md[0].x, 0.0);
    assert_close(md[1].x, 800.0 / 3.0);
    assert_close(md[2].x, 1600.0 / 3.0);
    assert_close(md[0].w, 800.0 / 3.0);
    assert_close(md[1].y, 0.0);
}

#[test]
fn responsive_grid_applies_offset_and_visual_order_without_reordering_children() {
    let frames = responsive_child_frames(
        Grid::responsive()
            .cols(vec![
                Col::new().span(6).offset(6).order(2),
                Col::new().span(6).order(1),
            ])
            .justify(JustifyContent::Stretch),
        Rect::new(0.0, 0.0, 960.0, 100.0),
        2,
    );

    assert_close(frames[0].x, 480.0);
    assert_close(frames[1].x, 0.0);
    assert_close(frames[0].w, 240.0);
    assert_close(frames[1].w, 240.0);
}

#[test]
fn explicit_columns_disable_responsive_configuration() {
    let grid = Grid::responsive()
        .cols(vec![Col::new().span(12)])
        .columns(vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)]);

    assert!(!grid.is_responsive());
    assert_eq!(grid.style.grid_template_columns.len(), 2);
}
