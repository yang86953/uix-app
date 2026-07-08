use super::*;
use crate::core::{Point, Rect};
use crate::ui::core::widget::WidgetCore;
use crate::ui::traits::EventHandler;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

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

    let (frame, dx, dy) = tree
        .drain_scroll_region_move()
        .expect("table scroll should register memmove");
    assert_eq!(frame, Rect::new(0.0, 0.0, 320.0, 120.0));
    assert_eq!(dx, 0.0);
    assert_eq!(dy, 40.0);
}
