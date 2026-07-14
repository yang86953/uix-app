use crate::tests::common::*;
use crate::ui::traits::{EventHandler, WidgetAnimation};
use crate::ui::widgets::feedback::message::*;
use crate::ui::{AnimationConfig, EventResult, Placement, SystemEvent};

#[test]
fn measure_preserves_message_zero_layout_footprint() {
    let measured = Message::new().measure(Constraints::loose(Size::new(200.0, 80.0)));

    assert_eq!(measured, Size::zero());
}

#[test]
fn empty_message_does_not_block_hit_test() {
    let message = Message::new();
    let frame = Rect::new(0.0, 0.0, 800.0, 600.0);

    assert_eq!(message.hit_bounds(frame), None);
}

#[test]
fn bottom_left_message_stack_stays_inside_window() {
    let message = Message::new().placement(Placement::BottomLeft);
    message.info("first");
    message.success("second");

    let bounds = message
        .hit_bounds(Rect::new(0.0, 0.0, 800.0, 600.0))
        .expect("可见消息应有命中区域");

    assert_eq!(bounds, Rect::new(12.0, 500.0, 380.0, 88.0));
}

#[test]
fn every_message_placement_anchors_inside_window() {
    let cases = [
        (Placement::Top, Point::new(210.0, 12.0)),
        (Placement::TopLeft, Point::new(12.0, 12.0)),
        (Placement::TopRight, Point::new(408.0, 12.0)),
        (Placement::Bottom, Point::new(210.0, 548.0)),
        (Placement::BottomLeft, Point::new(12.0, 548.0)),
        (Placement::BottomRight, Point::new(408.0, 548.0)),
        (Placement::Left, Point::new(12.0, 280.0)),
        (Placement::Right, Point::new(408.0, 280.0)),
    ];

    for (placement, expected_origin) in cases {
        let message = Message::new().placement(placement);
        message.info("visible");
        let bounds = message
            .hit_bounds(Rect::new(0.0, 0.0, 800.0, 600.0))
            .expect("可见消息应有命中区域");

        assert_eq!(Point::new(bounds.x, bounds.y), expected_origin);
        assert_eq!(Size::new(bounds.w, bounds.h), Size::new(380.0, 40.0));
    }
}

#[test]
fn handle_keeps_queue_identity_and_dismisses_by_id() {
    let message = Message::new();
    let handle = message.handle();
    let id = handle.info("saved");

    assert_eq!(handle.items()[0].content, "saved");
    assert!(message.dismiss(id));
    assert!(handle.is_empty());
}

#[test]
fn message_holds_without_frames_and_expires_on_one_timer() {
    let mut message = Message::new();
    message.success("saved");

    assert!(!WidgetAnimation::update_animation(&mut message, 0.2));
    let (timer_id, delay) = EventHandler::active_timer(&message).expect("message timer");
    assert_eq!(delay, std::time::Duration::from_secs(3));

    assert_eq!(
        EventHandler::on_event(
            &mut message,
            &SystemEvent::Timer {
                id: timer_id as u32,
            },
        ),
        EventResult::Handled
    );
    assert!(message.items().is_empty());
    assert!(WidgetAnimation::update_animation(&mut message, 0.1));
    assert!(!WidgetAnimation::update_animation(&mut message, 0.05));
}

#[test]
fn close_hit_starts_leave_before_removing_painted_item() {
    let mut message = Message::new().leave_animation(AnimationConfig::fade_out(0.2));
    let id = message.add(MessageItem {
        type_: crate::native::traits::system::StatusLevel::Info,
        content: "persistent".into(),
        duration_ms: 0,
        closable: true,
    });
    WidgetAnimation::update_animation(&mut message, 0.2);
    let frame = Rect::new(0.0, 0.0, 800.0, 600.0);
    let bounds = message.hit_bounds(frame).expect("visible message");

    assert_eq!(
        EventHandler::on_event(
            &mut message,
            &SystemEvent::PointerDown {
                pos: Point::new(bounds.x + bounds.w - 8.0, bounds.y + bounds.h * 0.5),
                button: MouseButton::Left,
                mods: crate::ui::KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );
    assert!(!message.dismiss(id));
    assert!(message.hit_bounds(frame).is_some());
    assert!(!WidgetAnimation::update_animation(&mut message, 0.2));
    assert!(message.hit_bounds(frame).is_none());
}

#[test]
fn slide_motion_dirty_bounds_cover_full_sweep_not_whole_window() {
    let mut message =
        Message::new().enter_animation(AnimationConfig::slide_in(Placement::Top, 0.2));
    message.info("moving");
    assert!(WidgetAnimation::update_animation(&mut message, 0.1));

    let frame = Rect::new(0.0, 0.0, 800.0, 600.0);
    let dirty = WidgetAnimation::dirty_bounds(&message, frame);

    assert!(dirty.contains(Point::new(400.0, -10.0)));
    assert!(dirty.w < frame.w);
    assert!(dirty.h < 100.0);
}
