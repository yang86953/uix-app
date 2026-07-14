use crate::ui::widgets::display::table::geometry::TableColumnGeometry;
use crate::ui::widgets::{Fixed, TableColumn};

fn columns() -> Vec<TableColumn> {
    vec![
        TableColumn::new("Name", 80.0).fixed(Fixed::Left),
        TableColumn::new("Email", 160.0),
        TableColumn::new("Role", 120.0),
        TableColumn::new("Actions", 60.0).fixed(Fixed::Right),
    ]
}

#[test]
fn fixed_columns_stay_anchored_while_middle_columns_scroll() {
    let initial = TableColumnGeometry::new(&columns(), 0.0, 240.0, 0.0, 0.0);
    let scrolled = TableColumnGeometry::new(&columns(), 0.0, 240.0, 0.0, 80.0);

    assert_eq!(initial.columns[0].x, scrolled.columns[0].x);
    assert_eq!(initial.columns[3].x, scrolled.columns[3].x);
    assert_eq!(initial.columns[1].x - scrolled.columns[1].x, 80.0);
    assert_eq!(scrolled.max_scroll_x, 180.0);
}

#[test]
fn fixed_columns_win_hit_testing_over_clipped_middle_columns() {
    let geometry = TableColumnGeometry::new(&columns(), 0.0, 240.0, 0.0, 100.0);

    assert_eq!(geometry.column_at(20.0), Some(0));
    assert_eq!(geometry.column_at(220.0), Some(3));
    assert_eq!(geometry.column_at(100.0), Some(1));
}
