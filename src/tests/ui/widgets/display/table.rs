use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::view::adapter::ViewAdapter;
use crate::ui::widgets::display::table::*;

fn render_table(table: &Table, frame: Rect, surface_size: (i32, i32)) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::painting::DisplayList::new();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            font,
            &fonts,
            &images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            surface_size.0,
            surface_size.1,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(table, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn loading_table_keeps_header_visible_animates_and_rejects_interaction() {
    let mut table = Table::new()
        .columns(vec![TableColumn::new("Name", 120.0)])
        .rows(vec![vec!["Ada".into()]])
        .selection(true)
        .bordered(true)
        .loading(true)
        .size(120.0, 96.0);
    table.last_frame.set(Some(Rect::new(0.0, 0.0, 120.0, 96.0)));

    assert_eq!(WidgetComponent::tab_index(&table), 0);
    assert_eq!(
        table.on_event(&pointer_down(12.0, 48.0)),
        EventResult::Handled
    );
    assert!(table.checked_rows().is_empty());
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { loading: true, .. }
    ));

    assert!(WidgetAnimation::update_animation(&mut table, 0.1));
    let dirty = WidgetAnimation::dirty_bounds(&table, Rect::new(0.0, 0.0, 120.0, 96.0));
    assert!(dirty.w > 0.0 && dirty.h > 0.0);
    let display = render_table(&table, Rect::new(0.0, 0.0, 120.0, 96.0), (120, 96));
    assert!(
        display.contains("Name"),
        "header must remain recorded: {display}"
    );
    assert!(
        display.matches("FillCircle").count() >= 8,
        "spinner must record eight circle ops: {display}"
    );

    let mut ready = table.loading(false);
    assert_eq!(WidgetComponent::tab_index(&ready), 1);
    assert!(!WidgetAnimation::update_animation(&mut ready, 0.1));
}

#[test]
fn table_normalizes_invalid_dimensions_and_row_heights() {
    let mut invalid_column = TableColumn::new("Invalid", 40.0);
    invalid_column.width = f32::NAN;
    let table = Table::new()
        .columns(vec![invalid_column])
        .rows(vec![vec!["Value".into()]])
        .row_height(f32::NAN)
        .size(f32::INFINITY, -20.0);

    assert_eq!(
        table.measure(Constraints::loose(Size::new(500.0, 500.0))),
        Size::zero()
    );
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, row_h, .. }
            if columns[0].width == 0.0 && row_h == 28.0
    ));

    let one_pixel_rows = Table::new().row_height(-4.0);
    assert!(matches!(
        one_pixel_rows.snapshot_fields(),
        SnapshotFields::Table { row_h, .. } if row_h == 1.0
    ));
}

#[test]
fn resizable_column_drag_updates_snapshot_width_and_survives_reconcile() {
    let mut table = Table::new()
        .columns(vec![
            TableColumn::new("Name", 100.0).resizable(true),
            TableColumn::new("Role", 80.0),
        ])
        .rows(vec![vec!["Ada".into(), "Engineer".into()]])
        .size(180.0, 100.0);
    table
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 180.0, 100.0)));

    assert_eq!(
        table.on_event(&pointer_down(100.0, 16.0)),
        EventResult::Handled
    );
    assert!(EventHandler::wants_continuous_pointer_move(&table));
    assert_eq!(
        table.on_event(&SystemEvent::PointerMove {
            pos: Point::new(132.0, 16.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(EventHandler::take_layout_request(&mut table));
    assert_eq!(
        table.on_event(&pointer_up(132.0, 16.0)),
        EventResult::Handled
    );
    assert!(!EventHandler::wants_continuous_pointer_move(&table));
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, .. }
            if columns[0].resizable && columns[0].width == 132.0
    ));

    table.sync_from(
        Table::new()
            .columns(vec![
                TableColumn::new("Name", 100.0).resizable(true),
                TableColumn::new("Role", 80.0),
            ])
            .rows(vec![vec!["Ada".into(), "Engineer".into()]])
            .size(180.0, 100.0),
    );
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, .. } if columns[0].width == 132.0
    ));
}

#[test]
fn resizable_column_drag_cancels_on_leave_and_clamps_to_a_usable_minimum() {
    let mut table = Table::new()
        .columns(vec![TableColumn::new("Name", 100.0).resizable(true)])
        .rows(vec![vec!["Ada".into()]])
        .size(140.0, 100.0);
    table
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 140.0, 100.0)));

    assert_eq!(
        table.on_event(&pointer_down(100.0, 16.0)),
        EventResult::Handled
    );
    assert_eq!(
        table.on_event(&SystemEvent::PointerMove {
            pos: Point::new(120.0, 16.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        table.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, .. } if columns[0].width == 100.0
    ));

    assert_eq!(
        table.on_event(&pointer_down(100.0, 16.0)),
        EventResult::Handled
    );
    assert_eq!(
        table.on_event(&SystemEvent::PointerMove {
            pos: Point::new(10.0, 16.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, .. } if columns[0].width == 32.0
    ));
    assert_eq!(
        table.on_event(&pointer_up(32.0, 16.0)),
        EventResult::Handled
    );
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, .. } if columns[0].width == 32.0
    ));
}

#[test]
fn constrained_table_clips_and_elides_headers_cells_and_empty_text() {
    let table = Table::new()
        .columns(vec![
            TableColumn::new("一个很长的可排序组件标题", 74.0).sortable(true),
            TableColumn::new("Semantic identifier heading", 76.0),
        ])
        .rows(vec![vec![
            "一段很长的中英文单元格内容 mixed value".into(),
            "another very long table cell value".into(),
        ]])
        .bordered(true);
    let display_list = render_table(&table, Rect::new(10.0, 8.0, 150.0, 64.0), (180, 90));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 8.0, w: 150.0, h: 64.0 } }"),
        "{display_list}"
    );
    assert!(display_list.contains('…'), "{display_list}");
    assert!(
        !display_list.contains("w: -") && !display_list.contains("h: -"),
        "{display_list}"
    );

    let empty = Table::new()
        .empty_text("这里是一段很长的自定义空表提示")
        .bordered(true);
    let empty_display = render_table(&empty, Rect::new(0.0, 0.0, 80.0, 24.0), (100, 40));
    assert!(empty_display.contains('…'), "{empty_display}");
    assert!(!empty_display.contains("NaN"), "{empty_display}");
}

#[test]
fn table_does_not_hit_rows_outside_the_actual_body_viewport() {
    let mut table = Table::new()
        .columns(vec![TableColumn::new("Name", 120.0)])
        .rows(vec![vec!["Ada".into()]])
        .size(120.0, 20.0);
    table.last_frame.set(Some(Rect::new(0.0, 0.0, 120.0, 20.0)));

    assert_eq!(table.body_viewport_height(), 0.0);
    assert_eq!(
        table.on_event(&pointer_down(20.0, 40.0)),
        EventResult::NotHandled
    );
    assert_eq!(table.selected_row(), None);
}

#[test]
fn populated_table_is_focusable_and_tracks_focus_state() {
    let mut table = Table::new()
        .columns(vec![TableColumn::new("Name", 120.0)])
        .rows(vec![vec!["UIX".into()]]);

    assert_eq!(WidgetComponent::tab_index(&table), 1);
    assert_eq!(table.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(table.on_event(&SystemEvent::FocusOut), EventResult::Handled);
    assert_eq!(WidgetComponent::tab_index(&Table::new()), 0);
}
use crate::ui::SnapshotTableColumnGroup;

fn pointer_down(x: f32, y: f32) -> SystemEvent {
    SystemEvent::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

fn pointer_up(x: f32, y: f32) -> SystemEvent {
    SystemEvent::PointerUp {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

fn click(table: &mut Table, x: f32, y: f32) -> EventResult {
    let down = table.on_event(&pointer_down(x, y));
    if down == EventResult::NotHandled {
        return down;
    }
    table.on_event(&pointer_up(x, y))
}

fn click_tree_to(tree: &mut WidgetTree, target: ComponentId, x: f32, y: f32) -> EventResult {
    let down = tree.dispatch_to(target, &pointer_down(x, y));
    if down == EventResult::NotHandled {
        return down;
    }
    tree.dispatch_to(target, &pointer_up(x, y))
}

#[test]
fn fixed_viewport_size_bounds_long_table_measurement() {
    let table = Table::new()
        .columns(vec![TableColumn::new("Name", 120.0)])
        .rows((0..60).map(|index| vec![format!("User {index}")]).collect())
        .size(320.0, 200.0);

    assert_eq!(
        table.measure(Constraints::loose(Size::new(1000.0, 1000.0))),
        Size::new(320.0, 200.0)
    );
}

#[test]
fn table_level_sorting_cycles_and_survives_reconcile() {
    let mut table = Table::new()
        .columns(vec![TableColumn::new("Name", 120.0)])
        .rows(vec![vec!["Ada".to_owned()]])
        .sortable(true);

    assert_eq!(click(&mut table, 10.0, 10.0), EventResult::Handled);
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

    let _ = click(&mut table, 10.0, 10.0);
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
    assert_eq!(click(&mut plain, 8.0, 40.0), EventResult::Handled);
    assert_eq!(plain.selected_row(), Some(0));
    assert!(plain.checked_rows().is_empty());

    let mut selectable = Table::new().rows(rows).selection(true);
    assert_eq!(click(&mut selectable, 8.0, 40.0), EventResult::Handled);
    assert_eq!(selectable.checked_rows(), &[0]);
    let _ = click(&mut selectable, 8.0, 10.0);
    assert_eq!(selectable.checked_rows(), &[0, 1]);
    let _ = click(&mut selectable, 8.0, 10.0);
    assert!(selectable.checked_rows().is_empty());
}

#[test]
fn table_pointer_actions_commit_only_after_matching_release() {
    let mut table = Table::new()
        .columns(vec![TableColumn::new("Name", 120.0).sortable(true)])
        .rows(vec![vec!["Ada".into()]])
        .sortable(true)
        .selection(true)
        .size(180.0, 100.0);
    table
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 180.0, 100.0)));

    assert_eq!(
        table.on_event(&pointer_down(60.0, 10.0)),
        EventResult::Handled
    );
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, .. }
            if columns[0].sort_direction == SortDirection::None
    ));
    assert_eq!(
        table.on_event(&pointer_up(190.0, 10.0)),
        EventResult::Handled
    );
    assert!(matches!(
        table.snapshot_fields(),
        SnapshotFields::Table { columns, .. }
            if columns[0].sort_direction == SortDirection::None
    ));

    assert_eq!(
        table.on_event(&pointer_down(8.0, 44.0)),
        EventResult::Handled
    );
    assert!(table.checked_rows().is_empty());
    assert_eq!(table.on_event(&pointer_up(8.0, 44.0)), EventResult::Handled);
    assert_eq!(table.checked_rows(), &[0]);

    assert_eq!(
        table.on_event(&pointer_down(60.0, 44.0)),
        EventResult::Handled
    );
    assert_eq!(table.selected_row(), None);
    assert_eq!(
        table.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(
        table.on_event(&pointer_up(60.0, 44.0)),
        EventResult::NotHandled
    );
    assert_eq!(table.selected_row(), None);
}

#[test]
fn table_hover_and_wheel_are_limited_to_the_actual_body_viewport() {
    let mut table = Table::new()
        .columns(vec![TableColumn::new("Name", 120.0)])
        .rows((0..20).map(|index| vec![format!("Row {index}")]).collect())
        .size(120.0, 100.0)
        .virtual_scroll(true);
    table
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 120.0, 100.0)));

    let header_wheel = SystemEvent::Wheel {
        pos: Point::new(60.0, 10.0),
        delta: Point::new(0.0, 1.0),
    };
    assert_eq!(table.on_event(&header_wheel), EventResult::NotHandled);
    assert_eq!(table.body_scroll.scroll_offset(), 0.0);

    let body_wheel = SystemEvent::Wheel {
        pos: Point::new(60.0, 60.0),
        delta: Point::new(0.0, 1.0),
    };
    assert_eq!(table.on_event(&body_wheel), EventResult::Handled);
    assert!(table.body_scroll.scroll_offset() > 0.0);

    let move_over_row = SystemEvent::PointerMove {
        pos: Point::new(60.0, 44.0),
        mods: KeyMod::NONE,
    };
    assert_eq!(table.on_event(&move_over_row), EventResult::Handled);
    assert_eq!(table.on_event(&move_over_row), EventResult::NotHandled);
    assert_eq!(
        table.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(
        table.on_event(&SystemEvent::PointerLeave),
        EventResult::NotHandled
    );
}

#[test]
fn expandable_builder_forwards_documented_table_flags() {
    let tree = ViewAdapter::build(
        Table::new()
            .rows(vec![vec!["Ada".to_owned()]])
            .expandable(48.0, |_row| crate::ui::view::label("Details"))
            .sortable(true)
            .selection(true)
            .bordered(true)
            .loading(true),
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
            loading: true,
            virtual_scroll: false,
            ..
        }
    ));
}

#[test]
fn typed_table_forwards_loading_state() {
    let tree = ViewAdapter::build(
        Table::data(
            vec![UserRow {
                id: 10,
                name: "Ada".to_string(),
                age: 28,
            }],
            |row| row.id.to_string(),
        )
        .expect("unique row")
        .columns(vec![
            TableColumn::new("Name", 120.0).bind(|row: &UserRow| row.name.clone())
        ])
        .loading(true),
    );
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
        SnapshotFields::Table { loading: true, .. }
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
            delta: Point::new(2.0, 0.0),
        }),
        EventResult::Handled
    );
    assert_eq!(table.horizontal_scroll_offset(), 80.0);
    assert_eq!(EventHandler::scroll_delta_for_dirty(&table), None);

    assert_eq!(click(&mut table, 220.0, 10.0), EventResult::Handled);
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

    assert_eq!(click(&mut table, 20.0, 10.0), EventResult::NotHandled);
    assert_eq!(click(&mut table, 20.0, 42.0), EventResult::Handled);
    assert_eq!(click(&mut table, 200.0, 10.0), EventResult::Handled);
    assert_eq!(click(&mut table, 20.0, 70.0), EventResult::Handled);
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

fn interactive_user_table(rows: Vec<UserRow>, clicks: Rc<Cell<usize>>) -> DataTable<UserRow> {
    Table::data(rows, |row| row.id.to_string())
        .expect("user ids should be unique")
        .columns(vec![
            TableColumn::new("Name", 120.0).bind(|row: &UserRow| row.name.clone()),
            TableColumn::new("Action", 100.0)
                .bind(|row: &UserRow| format!("Edit {}", row.name))
                .render(move |row: &UserRow| {
                    let clicks = Rc::clone(&clicks);
                    crate::ui::view::button(format!("Edit {}", row.name)).on_click_fn(move || {
                        clicks.set(clicks.get() + 1);
                    })
                }),
        ])
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
        click_tree_to(&mut tree, root, 8.0, 70.0),
        EventResult::Handled
    );
    assert_eq!(
        click_tree_to(&mut tree, root, 48.0, 70.0),
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

#[test]
fn typed_table_view_cell_materializes_interactive_keyed_child() {
    use crate::ui::widgets::Button;

    let clicks = Rc::new(Cell::new(0));
    let mut tree = ViewAdapter::build(interactive_user_table(
        vec![UserRow {
            id: 10,
            name: "Ada".to_string(),
            age: 28,
        }],
        Rc::clone(&clicks),
    ));
    let root = tree.root_id().expect("typed table root");
    tree.get_mut(root)
        .expect("typed table node")
        .set_frame(Rect::new(0.0, 0.0, 220.0, 100.0));
    tree.layout();

    assert!(tree.has_table_cell_renderer(root));
    let child = tree
        .get(root)
        .expect("typed table node")
        .children()
        .first()
        .copied()
        .expect("view cell child");
    let child_node = tree.get(child).expect("view cell node");
    assert_eq!(child_node.key(), Some("table-cell:10:1"));
    assert_eq!(
        child_node
            .component()
            .as_any()
            .downcast_ref::<Button>()
            .expect("button cell")
            .text(),
        "Edit Ada"
    );
    assert_eq!(child_node.frame(), Rect::new(120.0, 33.0, 100.0, 28.0));
    assert!(matches!(
        tree.get(root)
            .expect("typed table node")
            .component()
            .snapshot_fields(),
        SnapshotFields::Table {
            rows,
            view_columns,
            ..
        } if rows == vec![vec!["Ada".to_string(), "Edit Ada".to_string()]]
            && view_columns == vec![1]
    ));

    let position = Point::new(160.0, 45.0);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: position,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: position,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(clicks.get(), 1);
}

#[test]
fn typed_table_view_cells_reconcile_by_row_key_after_reorder() {
    use crate::ui::widgets::Button;

    let clicks = Rc::new(Cell::new(0));
    let mut tree = ViewAdapter::build(interactive_user_table(
        vec![
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
        ],
        Rc::clone(&clicks),
    ));
    let root = tree.root_id().expect("typed table root");
    let original = tree
        .get(root)
        .expect("typed table node")
        .children()
        .to_vec();
    assert_eq!(original.len(), 2);

    ViewAdapter::reconcile(
        &mut tree,
        interactive_user_table(
            vec![
                UserRow {
                    id: 20,
                    name: "Grace Hopper".to_string(),
                    age: 36,
                },
                UserRow {
                    id: 10,
                    name: "Ada".to_string(),
                    age: 28,
                },
            ],
            clicks,
        ),
    );

    let reordered = tree
        .get(root)
        .expect("reconciled typed table")
        .children()
        .to_vec();
    assert_eq!(reordered, vec![original[1], original[0]]);
    assert_eq!(
        tree.get(reordered[0])
            .expect("reused Grace cell")
            .component()
            .as_any()
            .downcast_ref::<Button>()
            .expect("button cell")
            .text(),
        "Edit Grace Hopper"
    );
}

#[test]
fn typed_table_view_cells_follow_fixed_column_zones_while_scrolling() {
    let table = Table::data(
        vec![UserRow {
            id: 10,
            name: "Ada".to_string(),
            age: 28,
        }],
        |row| row.id.to_string(),
    )
    .expect("row id is unique")
    .columns(vec![
        TableColumn::new("Name", 80.0)
            .fixed(Fixed::Left)
            .bind(|row: &UserRow| row.name.clone()),
        TableColumn::new("Age", 100.0)
            .bind(|row: &UserRow| row.age.to_string())
            .render(|row: &UserRow| crate::ui::view::button(row.age.to_string())),
        TableColumn::new("Details", 100.0)
            .bind(|row: &UserRow| row.name.clone())
            .render(|row: &UserRow| crate::ui::view::button(row.name.clone())),
        TableColumn::new("Action", 80.0)
            .fixed(Fixed::Right)
            .bind(|row: &UserRow| format!("Edit {}", row.name))
            .render(|row: &UserRow| crate::ui::view::button(format!("Edit {}", row.name))),
    ]);
    let mut tree = ViewAdapter::build(table);
    let root = tree.root_id().expect("typed table root");
    tree.get_mut(root)
        .expect("typed table node")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 100.0));
    tree.layout();
    let children = tree
        .get(root)
        .expect("typed table node")
        .children()
        .to_vec();
    assert_eq!(children.len(), 3);
    assert_eq!(tree.get(children[0]).expect("age cell").frame().w, 80.0);
    assert_eq!(tree.get(children[1]).expect("details cell").frame().w, 0.0);
    assert_eq!(
        tree.get(children[2]).expect("fixed action cell").frame(),
        Rect::new(160.0, 33.0, 80.0, 28.0)
    );

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: Point::new(120.0, 45.0),
            delta: Point::new(2.0, 0.0),
        }),
        EventResult::Handled
    );
    tree.layout();

    assert_eq!(
        tree.get(children[0]).expect("scrolled age cell").frame().w,
        20.0
    );
    assert_eq!(
        tree.get(children[1])
            .expect("scrolled details cell")
            .frame(),
        Rect::new(100.0, 33.0, 60.0, 28.0)
    );
    assert_eq!(
        tree.get(children[2]).expect("fixed action cell").frame(),
        Rect::new(160.0, 33.0, 80.0, 28.0)
    );
}
