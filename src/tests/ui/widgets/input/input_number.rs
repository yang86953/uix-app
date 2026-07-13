use crate::tests::common::*;
use crate::component;
use crate::draw::{ Radius };
use crate::ui::widgets::input::input_number::*;

#[test]
fn measure_clamps_input_number_size() {
    let measured = InputNumber::new("0").measure(Constraints::loose(Size::new(60.0, 24.0)));

    assert_eq!(measured, Size::new(60.0, 24.0));
}

#[test]
fn uncontrolled_value_survives_reconcile() {

    let mut input = InputNumber::new("Count").min(0.0).max(100.0).step(1.0);
    let _ = input.on_event(&SystemEvent::PointerDown {
        pos: Point::new(1.0, 1.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Up,
        mods: KeyMod::NONE,
    });

    input.sync_from(InputNumber::new("Count").min(0.0).max(100.0).step(1.0));

    assert_eq!(input.get_value(), 1.0);
}
