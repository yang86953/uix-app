use super::*;
use crate::ui::foundation::style::Style;
use crate::ui::traits::WidgetLayout;
use crate::ui::widgets::Container;

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
