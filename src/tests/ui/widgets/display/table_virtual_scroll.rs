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
        .row_height(28.0)
}

#[test]
fn table_scroll_range_limits_visible_rows() {
    let table = large_table();
    table
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 320.0, 120.0)));
    let viewport_h = table.body_viewport_height();
    let (start, end) = table
        .body_scroll
        .scroll_range(table.rows.len(), table.row_h, viewport_h);
    assert_eq!(start, 0);
    assert!(
        end - start < 100,
        "virtual scroll should expose a small window"
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
    assert_eq!(frame, Rect::new(0.0, 0.0, 320.0, 120.0));
    assert_eq!(dx, 0.0);
    assert_eq!(dy, 40.0);
}

#[test]
fn table_expand_renderer_is_invoked_from_paint_sidecar() {
    use crate::draw::compositor::ScenePaint;
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
    use crate::draw::spatial::Orientation;
    use crate::ui::view::adapter::ViewAdapter;

    let calls = Rc::new(Cell::new(0));
    let renderer_calls = Rc::clone(&calls);
    let mut tree = ViewAdapter::build(
        Table::new()
            .columns(vec![TableColumn::new("Name", 320.0)])
            .rows(vec![vec!["Ada".to_string()]])
            .expandable(48.0, move |row, _ctx, _rect| {
                assert_eq!(row, 0);
                renderer_calls.set(renderer_calls.get() + 1);
            }),
    );
    let id = tree.root_id().expect("table root");
    let frame = Rect::new(0.0, 0.0, 320.0, 120.0);
    tree.get_mut(id).unwrap().set_frame(frame);
    {
        let table = tree
            .get_mut(id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Table>()
            .unwrap();
        assert_eq!(
            EventHandler::on_event(
                table,
                &SystemEvent::PointerDown {
                    pos: Point::new(310.0, 40.0),
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                },
            ),
            EventResult::Handled
        );
        assert_eq!(table.expanded_row(), Some(0));
    }

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
}
