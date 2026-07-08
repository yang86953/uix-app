use super::*;
use crate::core::{Point, Rect};
use crate::ui::core::widget::WidgetCore;
use crate::ui::traits::EventHandler;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

fn long_selectable_list() -> SelectableList {
    let mut list = SelectableList::new();
    list.items = (0..100)
        .map(|i| SelectableItem::new(format!("item-{i}"), format!("Item {i}")))
        .collect();
    list
}

#[test]
fn selectable_list_scroll_range_limits_visible_rows() {
    let list = long_selectable_list();
    list.last_frame
        .set(Some(Rect::new(0.0, 0.0, 220.0, 120.0)));
    let viewport_h = list.list_body_viewport_height();
    let (start, end) = list
        .body_scroll
        .scroll_range(list.items.len(), list.item_stride(), viewport_h);
    assert_eq!(start, 0);
    assert!(end - start < 100, "virtual scroll should expose a small window");
}

#[test]
fn selectable_list_wheel_records_composite_delta() {
    let mut list = long_selectable_list();
    list.last_frame
        .set(Some(Rect::new(0.0, 0.0, 220.0, 120.0)));

    assert_eq!(
        EventHandler::on_event(
            &mut list,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 10.0),
                delta: Point::new(0.0, -1.0),
            },
        ),
        EventResult::Handled
    );
    assert!(list.body_scroll.scroll_offset() > 0.0);
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&list),
        Some((0.0, 40.0))
    );
    assert!(EventHandler::scroll_delta_for_dirty(&list).is_none());
}

#[test]
fn selectable_list_wheel_at_scroll_boundary_does_not_record_delta() {
    let mut list = long_selectable_list();
    list.last_frame
        .set(Some(Rect::new(0.0, 0.0, 220.0, 120.0)));

    let viewport_h = list.list_body_viewport_height();
    let max_offset = list.body_scroll.max_scroll_offset(
        list.items.len(),
        list.item_stride(),
        viewport_h,
    );
    list.body_scroll.set_scroll_offset(max_offset);

    assert_eq!(
        EventHandler::on_event(
            &mut list,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 10.0),
                delta: Point::new(0.0, -1.0),
            },
        ),
        EventResult::NotHandled
    );
    assert_eq!(list.body_scroll.scroll_offset(), max_offset);
    assert!(EventHandler::scroll_delta_for_dirty(&list).is_none());
}

#[test]
fn selectable_list_wheel_registers_composite_scroll_strip() {
    let mut tree = WidgetTree::new();
    let mut list = long_selectable_list();
    list.last_frame
        .set(Some(Rect::new(0.0, 0.0, 220.0, 120.0)));
    let id = tree.set_root(Box::new(list));
    tree.get_mut(id)
        .expect("selectable list root")
        .set_frame(Rect::new(0.0, 0.0, 220.0, 120.0));
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: Point::new(20.0, 20.0),
            delta: Point::new(0.0, -1.0),
        }),
        EventResult::Handled
    );

    let (frame, dx, dy) = tree
        .drain_scroll_region_move()
        .expect("selectable list scroll should register memmove");
    assert_eq!(frame, Rect::new(0.0, 0.0, 220.0, 120.0));
    assert_eq!(dx, 0.0);
    assert_eq!(dy, 40.0);
}

#[test]
fn selectable_list_row_index_accounts_for_scroll_offset() {
    let mut list = long_selectable_list();
    list.last_frame
        .set(Some(Rect::new(0.0, 0.0, 220.0, 120.0)));
    list.body_scroll.set_scroll_offset(list.item_stride() * 5.0);

    assert_eq!(list.row_index_at_y(10.0), Some(5));
    assert_eq!(list.row_index_at_y(10.0 + list.item_stride()), Some(6));
}
