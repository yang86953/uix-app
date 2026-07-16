use crate::tests::common::*;
use crate::ui::view::adapter::ViewAdapter;
use crate::ui::widgets::display::table::*;
use crate::ui::SnapshotTableColumnGroup;

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
            .expandable(48.0, |_row| crate::ui::view::label("Details"))
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
            virtual_scroll: false,
            ..
        }
    ));
}

#[test]
fn expandable_builder_forwards_virtual_scroll_policy() {
    let tree = ViewAdapter::build(
        Table::new()
            .rows(vec![vec!["Ada".to_owned()]])
            .expandable(48.0, |_row| crate::ui::view::label("Details"))
            .virtual_scroll(true)
            .virtual_row_height(36.0),
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
            row_h: 36.0,
            virtual_scroll: true,
            ..
        }
    ));
}

#[test]
fn fixed_columns_stay_hittable_after_horizontal_scroll() {
    let mut table = Table::new()
        .columns(vec![
            TableColumn::new("Name", 80.0)
                .fixed(Fixed::Left)
                .sortable(true),
            TableColumn::new("Email", 160.0),
            TableColumn::new("Role", 120.0),
            TableColumn::new("Actions", 60.0)
                .fixed(Fixed::Right)
                .sortable(true),
        ])
        .rows(vec![vec![
            "Ada".to_owned(),
            "ada@example.com".to_owned(),
            "Admin".to_owned(),
            "Edit".to_owned(),
        ]]);
    table
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 240.0, 120.0)));

    assert_eq!(
        table.on_event(&SystemEvent::Wheel {
            pos: Point::new(120.0, 80.0),
            delta: Point::new(-2.0, 0.0),
        }),
        EventResult::Handled
    );
    assert_eq!(table.horizontal_scroll_offset(), 80.0);
    assert_eq!(EventHandler::scroll_delta_for_dirty(&table), None);

    assert_eq!(table.on_event(&click(220.0, 10.0)), EventResult::Handled);
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, .. }
            if columns[0].fixed == Some(Fixed::Left)
                && columns[3].fixed == Some(Fixed::Right)
                && columns[3].sort_direction == SortDirection::Asc
    ));
}

#[test]
fn column_groups_create_two_level_headers_and_keep_leaf_sorting() {
    let mut table = Table::new()
        .column_groups(vec![
            TableColumnGroup::new(
                "Identity",
                vec![
                    TableColumn::new("Name", 100.0).sortable(true),
                    TableColumn::new("Age", 60.0),
                ],
            ),
            TableColumnGroup::column(TableColumn::new("Department", 100.0).sortable(true)),
        ])
        .rows(vec![vec!["Ada".into(), "36".into(), "Research".into()]]);

    assert_eq!(table.on_event(&click(20.0, 10.0)), EventResult::NotHandled);
    assert_eq!(table.on_event(&click(20.0, 42.0)), EventResult::Handled);
    assert_eq!(table.on_event(&click(200.0, 10.0)), EventResult::Handled);
    assert_eq!(table.on_event(&click(20.0, 70.0)), EventResult::Handled);
    assert_eq!(table.selected_row(), Some(0));

    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table {
            columns,
            column_groups,
            ..
        } if columns.len() == 3
            && columns[0].sort_direction == SortDirection::None
            && columns[2].sort_direction == SortDirection::Asc
            && column_groups == vec![
                SnapshotTableColumnGroup {
                    title: Some("Identity".to_owned()),
                    start: 0,
                    len: 2,
                },
                SnapshotTableColumnGroup {
                    title: None,
                    start: 2,
                    len: 1,
                },
            ]
    ));
}

#[derive(Debug)]
struct UserRow {
    id: u64,
    name: String,
    age: u32,
}

fn user_table(rows: Vec<UserRow>) -> DataTable<UserRow> {
    Table::data(rows, |row| row.id.to_string())
        .expect("user ids should be unique")
        .columns(vec![
            TableColumn::new("Name", 120.0).bind(|row: &UserRow| row.name.clone()),
            TableColumn::new("Age", 80.0).bind(|row: &UserRow| row.age.to_string()),
        ])
        .selection(true)
        .virtual_scroll(true)
}

#[test]
fn typed_table_rows_project_text_and_publish_stable_keys() {
    let tree = ViewAdapter::build(user_table(vec![
        UserRow {
            id: 10,
            name: "Ada".to_string(),
            age: 28,
        },
        UserRow {
            id: 20,
            name: "Grace".to_string(),
            age: 35,
        },
    ]));
    let root = tree.root_id().expect("typed table root");
    let table = tree
        .get(root)
        .expect("typed table node")
        .component()
        .as_any()
        .downcast_ref::<Table>()
        .expect("Table component");

    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table {
            rows,
            row_keys,
            virtual_scroll: true,
            ..
        } if rows == vec![
            vec!["Ada".to_string(), "28".to_string()],
            vec!["Grace".to_string(), "35".to_string()],
        ] && row_keys == vec!["10".to_string(), "20".to_string()]
    ));
}

#[test]
fn typed_table_reconcile_preserves_selection_by_row_key_after_reorder() {
    let mut tree = ViewAdapter::build(user_table(vec![
        UserRow {
            id: 10,
            name: "Ada".to_string(),
            age: 28,
        },
        UserRow {
            id: 20,
            name: "Grace".to_string(),
            age: 35,
        },
    ]));
    let root = tree.root_id().expect("typed table root");
    assert_eq!(
        tree.dispatch_to(root, &click(8.0, 70.0)),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_to(root, &click(48.0, 70.0)),
        EventResult::Handled
    );

    ViewAdapter::reconcile(
        &mut tree,
        user_table(vec![
            UserRow {
                id: 20,
                name: "Grace".to_string(),
                age: 36,
            },
            UserRow {
                id: 10,
                name: "Ada".to_string(),
                age: 28,
            },
        ]),
    );

    let table = tree
        .get(root)
        .expect("reconciled typed table")
        .component()
        .as_any()
        .downcast_ref::<Table>()
        .expect("Table component");
    assert_eq!(table.selected_row(), Some(0));
    assert_eq!(table.selected_row_key(), Some("20"));
    assert_eq!(table.checked_rows(), &[0]);
    assert_eq!(table.checked_row_keys(), vec!["20"]);
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { rows, .. } if rows[0][1] == "36"
    ));
}

#[test]
fn typed_table_rejects_duplicate_row_keys_before_build() {
    let result = Table::data(
        vec![
            UserRow {
                id: 10,
                name: "Ada".to_string(),
                age: 28,
            },
            UserRow {
                id: 10,
                name: "Duplicate".to_string(),
                age: 30,
            },
        ],
        |row| row.id.to_string(),
    );

    match result {
        Err(error) => assert_eq!(error, TableDataError::DuplicateRowKey("10".to_string())),
        Ok(_) => panic!("duplicate row key should fail"),
    }
}
