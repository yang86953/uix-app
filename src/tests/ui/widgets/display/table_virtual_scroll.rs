use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::display::table::*;

fn large_table() -> Table {
    let rows: Vec<Vec<String>> = (0..100)
        .map(|i| vec![format!("User {i}"), format!("Role {i}"), "Active".into()])
        .collect();
    Table::new()
        .columns(vec![
            TableColumn::new("Name", 120.0),
            TableColumn::new("Role", 120.0),
            TableColumn::new("Status", 80.0),
        ])
        .rows(rows)
        .virtual_scroll(true)
        .virtual_row_height(28.0)
}

#[test]
fn table_scroll_range_limits_visible_rows() {
    let table = large_table();
    table
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 320.0, 120.0)));
    let viewport_h = table.body_viewport_height();
    let (start, end) = table.visible_row_range(viewport_h);
    assert_eq!(start, 0);
    assert!(
        end - start < 100,
        "virtual scroll should expose a small window"
    );
}

#[test]
fn table_without_virtual_scroll_keeps_the_full_row_range() {
    let rows = (0..100).map(|i| vec![i.to_string()]).collect::<Vec<_>>();
    let table = Table::new()
        .columns(vec![TableColumn::new("Value", 120.0)])
        .rows(rows);

    assert_eq!(table.visible_row_range(120.0), (0, 100));
}

#[test]
fn typed_view_cells_materialize_only_the_virtual_row_window() {
    use crate::ui::view::{button, ViewAdapter};

    #[derive(Debug)]
    struct Row {
        id: usize,
        label: String,
    }

    let rows = (0..100)
        .map(|id| Row {
            id,
            label: format!("Row {id}"),
        })
        .collect::<Vec<_>>();
    let calls = Rc::new(Cell::new(0));
    let renderer_calls = Rc::clone(&calls);
    let table = Table::data(rows, |row| row.id.to_string())
        .expect("row ids are unique")
        .columns(vec![TableColumn::new("Action", 160.0)
            .bind(|row: &Row| row.label.clone())
            .render(move |row: &Row| {
                renderer_calls.set(renderer_calls.get() + 1);
                button(row.label.clone())
            })])
        .virtual_scroll(true)
        .virtual_row_height(28.0);
    let mut tree = ViewAdapter::build(table);
    let root = tree.root_id().expect("typed table root");
    tree.get_mut(root)
        .expect("typed table node")
        .set_frame(Rect::new(0.0, 0.0, 160.0, 120.0));
    tree.layout();

    let (range, child_count) = {
        let node = tree.get(root).expect("typed table node");
        let table = node
            .component()
            .as_any()
            .downcast_ref::<Table>()
            .expect("Table component");
        (
            table.cell_view_range_for_frame(node.frame()),
            node.children().len(),
        )
    };
    assert_eq!(child_count, range.1 - range.0);
    assert!(child_count < 100);
    assert!(calls.get() < 100);
    let first_key = tree
        .get(root)
        .expect("typed table node")
        .children()
        .first()
        .and_then(|id| tree.get(*id))
        .and_then(|node| node.key())
        .expect("first cell key")
        .to_string();

    tree.reset_invalidation();
    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: Point::new(80.0, 80.0),
            delta: Point::new(0.0, -10.0),
        }),
        EventResult::Handled
    );
    tree.layout();

    let node = tree.get(root).expect("scrolled typed table node");
    let table = node
        .component()
        .as_any()
        .downcast_ref::<Table>()
        .expect("Table component");
    let scrolled_range = table.cell_view_range_for_frame(node.frame());
    assert!(scrolled_range.0 > range.0);
    assert_eq!(node.children().len(), scrolled_range.1 - scrolled_range.0);
    let next_key = node
        .children()
        .first()
        .and_then(|id| tree.get(*id))
        .and_then(|node| node.key())
        .expect("first scrolled cell key");
    assert_ne!(next_key, first_key);
    assert!(
        tree.scroll_region_moves().is_none(),
        "a changed View subtree must conservatively repaint instead of composite-copying"
    );
}

#[test]
fn table_wheel_records_composite_delta() {
    let mut table = large_table();
    table
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 320.0, 120.0)));

    assert_eq!(
        EventHandler::on_event(
            &mut table,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 80.0),
                delta: Point::new(0.0, -1.0),
            },
        ),
        EventResult::Handled
    );
    assert!(table.body_scroll.scroll_offset() > 0.0);
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&table),
        Some((0.0, 40.0))
    );
    assert!(EventHandler::scroll_delta_for_dirty(&table).is_none());
}

#[test]
fn table_wheel_registers_composite_scroll_strip() {
    let mut tree = WidgetTree::new();
    let table = large_table();
    table
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 320.0, 120.0)));
    let id = tree.set_root(Box::new(table));
    tree.get_mut(id)
        .expect("table root")
        .set_frame(Rect::new(0.0, 0.0, 320.0, 120.0));
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: Point::new(20.0, 80.0),
            delta: Point::new(0.0, -1.0),
        }),
        EventResult::Handled
    );

    let moves = tree
        .scroll_region_moves()
        .expect("table scroll should register memmove");
    assert_eq!(moves.len(), 1);
    let (frame, dx, dy) = moves[0];
    assert_eq!(frame, Rect::new(0.0, 33.0, 320.0, 87.0));
    assert_eq!(dx, 0.0);
    assert_eq!(dy, 40.0);
}

#[test]
fn grouped_table_scroll_excludes_both_header_rows_from_composite() {
    let mut tree = WidgetTree::new();
    let rows = (0..10)
        .map(|index| vec![format!("User {index}"), format!("Role {index}")])
        .collect();
    let table = Table::new()
        .column_groups(vec![TableColumnGroup::new(
            "Account",
            vec![
                TableColumn::new("Name", 160.0),
                TableColumn::new("Role", 160.0),
            ],
        )])
        .rows(rows);
    table
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 320.0, 160.0)));
    let id = tree.set_root(Box::new(table));
    tree.get_mut(id)
        .expect("table root")
        .set_frame(Rect::new(0.0, 0.0, 320.0, 160.0));
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: Point::new(20.0, 100.0),
            delta: Point::new(0.0, -1.0),
        }),
        EventResult::Handled
    );

    let moves = tree
        .scroll_region_moves()
        .expect("grouped table scroll should register memmove");
    assert_eq!(moves, vec![(Rect::new(0.0, 65.0, 320.0, 95.0), 0.0, 40.0)]);
}

#[test]
fn table_horizontal_wheel_falls_back_to_table_paint() {
    let mut tree = WidgetTree::new();
    let table = Table::new()
        .columns(vec![
            TableColumn::new("Name", 80.0).fixed(Fixed::Left),
            TableColumn::new("Email", 220.0),
            TableColumn::new("Actions", 80.0).fixed(Fixed::Right),
        ])
        .rows(vec![vec![
            "Ada".into(),
            "ada@example.com".into(),
            "Edit".into(),
        ]]);
    table
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 240.0, 120.0)));
    let id = tree.set_root(Box::new(table));
    tree.get_mut(id)
        .expect("table root")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 120.0));
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: Point::new(120.0, 80.0),
            delta: Point::new(-1.0, 0.0),
        }),
        EventResult::Handled
    );

    assert!(tree.scroll_region_moves().is_none());
    assert!(!tree.dirty_region().is_empty());
}

#[test]
fn table_expand_view_is_materialized_once_and_remains_interactive() {
    use crate::draw::compositor::ScenePaint;
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
    use crate::draw::spatial::Orientation;
    use crate::ui::view::{button, ViewAdapter};
    use crate::ui::widgets::Button;

    let calls = Rc::new(Cell::new(0));
    let renderer_calls = Rc::clone(&calls);
    let clicks = Rc::new(Cell::new(0));
    let renderer_clicks = Rc::clone(&clicks);
    let mut tree = ViewAdapter::build(
        Table::new()
            .columns(vec![TableColumn::new("Name", 320.0)])
            .rows(vec![vec!["Ada".to_string()]])
            .expandable(48.0, move |row| {
                assert_eq!(row, &["Ada".to_string()]);
                renderer_calls.set(renderer_calls.get() + 1);
                let button_clicks = Rc::clone(&renderer_clicks);
                button("Open Ada").on_click_fn(move || {
                    button_clicks.set(button_clicks.get() + 1);
                })
            }),
    );
    let id = tree.root_id().expect("table root");
    let frame = Rect::new(0.0, 0.0, 320.0, 120.0);
    tree.get_mut(id).expect("table node").set_frame(frame);
    tree.layout();
    let toggle_down = SystemEvent::PointerDown {
        pos: Point::new(310.0, 40.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    let toggle_up = SystemEvent::PointerUp {
        pos: Point::new(310.0, 40.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(tree.dispatch_event(&toggle_down), EventResult::Handled);
    assert_eq!(tree.dispatch_event(&toggle_up), EventResult::Handled);
    assert_eq!(calls.get(), 1);
    tree.layout();

    let child = tree
        .get(id)
        .expect("table node")
        .children()
        .first()
        .copied()
        .expect("expanded View child");
    let child_node = tree.get(child).expect("expanded child node");
    assert!(child_node.component().as_any().is::<Button>());
    assert_eq!(child_node.frame(), Rect::new(0.0, 61.0, 320.0, 48.0));

    let mut canvas = SharedRasterizer::new(PixelSurface::new(320, 120));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("deterministic font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        font,
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        320,
        120,
    );

    ScenePaint::paint(&tree, id, frame, &mut ctx);

    assert_eq!(calls.get(), 1);

    let button_pos = Point::new(20.0, 80.0);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: button_pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: button_pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(clicks.get(), 1);

    assert_eq!(tree.dispatch_event(&toggle_down), EventResult::Handled);
    assert_eq!(tree.dispatch_event(&toggle_up), EventResult::Handled);
    assert!(tree.get(id).expect("table node").children().is_empty());
}
