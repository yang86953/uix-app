use crate::tests::common::*;
use crate::ui::widgets::feedback::message::*;
use crate::ui::Placement;

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
