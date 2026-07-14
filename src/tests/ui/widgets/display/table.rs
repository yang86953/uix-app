use crate::tests::common::*;
use crate::ui::view::adapter::ViewAdapter;
use crate::ui::widgets::display::table::*;

fn click(x: f32, y: f32) -> SystemEvent {
    SystemEvent::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

#[test]
fn table_level_sorting_cycles_and_survives_reconcile() {
    let mut table = Table::new()
        .columns(vec![TableColumn::new("Name", 120.0)])
        .rows(vec![vec!["Ada".to_owned()]])
        .sortable(true);

    assert_eq!(table.on_event(&click(10.0, 10.0)), EventResult::Handled);
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, .. }
            if columns[0].sort_direction == SortDirection::Asc
    ));

    table.sync_from(
        Table::new()
            .columns(vec![TableColumn::new("Name", 120.0)])
            .rows(vec![vec!["Grace".to_owned()]])
            .sortable(true),
    );
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, .. }
            if columns[0].sort_direction == SortDirection::Asc
    ));

    let _ = table.on_event(&click(10.0, 10.0));
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, .. }
            if columns[0].sort_direction == SortDirection::Desc
    ));
}

#[test]
fn row_checkboxes_are_opt_in_and_header_toggles_all_rows() {
    let rows = vec![vec!["Ada".to_owned()], vec!["Grace".to_owned()]];
    let mut plain = Table::new().rows(rows.clone());
    assert_eq!(plain.on_event(&click(8.0, 40.0)), EventResult::Handled);
    assert_eq!(plain.selected_row(), Some(0));
    assert!(plain.checked_rows().is_empty());

    let mut selectable = Table::new().rows(rows).selection(true);
    assert_eq!(selectable.on_event(&click(8.0, 40.0)), EventResult::Handled);
    assert_eq!(selectable.checked_rows(), &[0]);
    let _ = selectable.on_event(&click(8.0, 10.0));
    assert_eq!(selectable.checked_rows(), &[0, 1]);
    let _ = selectable.on_event(&click(8.0, 10.0));
    assert!(selectable.checked_rows().is_empty());
}

#[test]
fn expandable_builder_forwards_documented_table_flags() {
    let tree = ViewAdapter::build(
        Table::new()
            .rows(vec![vec!["Ada".to_owned()]])
            .expandable(48.0, |_row, _ctx, _rect| {})
            .sortable(true)
            .selection(true)
            .bordered(true),
    );
    let root = tree.root_id().expect("table root");
    let table = tree
        .get(root)
        .expect("table node")
        .component()
        .as_any()
        .downcast_ref::<Table>()
        .expect("Table component");

    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table {
            sortable: true,
            selection: true,
            bordered: true,
            ..
        }
    ));
}
