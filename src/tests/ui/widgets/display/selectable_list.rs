use super::*;
use crate::core::{Point, Rect};
use crate::ui::core::widget::WidgetCore;
use crate::ui::traits::EventHandler;
use crate::ui::{EventResult, WidgetTree};

fn long_selectable_list() -> SelectableList {
    let mut list = SelectableList::new();
    list.items = (0..20)
        .map(|i| SelectableItem::new(format!("item-{i}"), format!("Item {i}")))
        .collect();
    list
}

#[test]
fn selectable_list_wheel_records_composite_delta_once() {
    let mut list = long_selectable_list();

    assert_eq!(
        EventHandler::on_event(
            &mut list,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 10.0),
                delta: Point::new(0.0, 100.0),
            },
        ),
        EventResult::Handled
    );

    assert_eq!(list.scroll_y.get(), -50.0);
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&list),
        Some((0.0, 50.0))
    );
    assert!(EventHandler::scroll_delta_for_dirty(&list).is_none());
}

#[test]
fn selectable_list_wheel_at_scroll_boundary_does_not_record_delta() {
    let mut list = long_selectable_list();

    assert_eq!(
        EventHandler::on_event(
            &mut list,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 10.0),
                delta: Point::new(0.0, 1000.0),
            },
        ),
        EventResult::Handled
    );
    let bottom = list.scroll_y.get();
    assert!(EventHandler::scroll_delta_for_dirty(&list).is_some());

    assert_eq!(
        EventHandler::on_event(
            &mut list,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 10.0),
                delta: Point::new(0.0, 100.0),
            },
        ),
        EventResult::NotHandled
    );
    assert_eq!(list.scroll_y.get(), bottom);
    assert!(EventHandler::scroll_delta_for_dirty(&list).is_none());
}

#[test]
fn selectable_list_wheel_registers_composite_scroll_strip() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(long_selectable_list()));
    tree.get_mut(id)
        .expect("selectable list root")
        .set_frame(Rect::new(0.0, 0.0, 220.0, 120.0));
    tree.reset_dirty();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: Point::new(20.0, 20.0),
            delta: Point::new(0.0, 100.0),
        }),
        EventResult::Handled
    );

    let dirty = tree.dirty_region();
    assert!(!dirty.full_frame);
    assert_eq!(dirty.rects(), &[Rect::new(0.0, 70.0, 220.0, 50.0)]);

    let (frame, dx, dy) = tree
        .drain_scroll_region_move()
        .expect("selectable list scroll should register memmove");
    assert_eq!(frame, Rect::new(0.0, 0.0, 220.0, 120.0));
    assert_eq!(dx, 0.0);
    assert_eq!(dy, 50.0);
}
