use super::*;
use crate::core::{Constraints, Point, Size};
use crate::ui::traits::{EventHandler, WidgetAnimation, WidgetLayout};
use crate::ui::{EventResult, SystemEvent};

fn large_select() -> Select {
    let opts: Vec<String> = (0..100).map(|i| format!("Option {i}")).collect();
    Select::new().options(opts)
}

#[test]
fn select_enter_animation_finishes_open() {
    let mut select = Select::new().options(vec!["A", "B"]);

    select.open();
    assert!(select.is_open());
    assert!(select.is_present());

    assert!(WidgetAnimation::update_animation(&mut select, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut select, 1.0));
    assert!(select.is_open());
    assert!(select.is_present());
}

#[test]
fn select_exit_animation_stays_present_until_finished() {
    let mut select = Select::new().options(vec!["A", "B"]);
    select.open();
    assert!(!WidgetAnimation::update_animation(&mut select, 1.0));

    select.close();
    assert!(!select.is_open());
    assert!(select.is_present());

    assert!(WidgetAnimation::update_animation(&mut select, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut select, 1.0));
    assert!(!select.is_open());
    assert!(!select.is_present());
}

#[test]
fn measure_clamps_select_size() {
    let measured = Select::new()
        .options(vec!["Alpha", "Beta"])
        .measure(Constraints::loose(Size::new(100.0, 24.0)));

    assert_eq!(measured, Size::new(100.0, 24.0));
}

#[test]
fn select_dropdown_scroll_range_limits_visible_rows() {
    let select = large_select();
    let row_count = select.dropdown_row_count();
    let viewport_h = select.dropdown_viewport_height(row_count);
    let (start, end) = select
        .dropdown_scroll
        .scroll_range(row_count, 28.0, viewport_h);
    assert_eq!(start, 0);
    assert!(end - start < 100, "virtual scroll should expose a small window");
}

#[test]
fn select_dropdown_wheel_records_composite_delta() {
    let mut select = large_select();
    select.open();

    assert_eq!(
        EventHandler::on_event(
            &mut select,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 50.0),
                delta: Point::new(0.0, -1.0),
            },
        ),
        EventResult::Handled
    );
    assert!(select.dropdown_scroll.scroll_offset() > 0.0);
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&select),
        Some((0.0, 40.0))
    );
}

#[test]
fn select_dropdown_row_at_y_accounts_for_scroll_offset() {
    let mut select = large_select();
    select.open();
    select.dropdown_scroll.set_scroll_offset(28.0 * 5.0);

    assert_eq!(select.dropdown_row_at_y(33.0), Some(5));
    assert_eq!(select.dropdown_row_at_y(61.0), Some(6));
}
