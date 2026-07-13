use crate::tests::common::*;
use crate::ui::widgets::general::label::*;

#[test]
fn measure_clamps_label_size() {
    let measured = Label::new("abcdef").measure(Constraints::loose(Size::new(30.0, 12.0)));

    assert_eq!(measured, Size::new(30.0, 12.0));
}

#[test]
fn label_ignores_pointer_when_not_selectable() {
    let mut label = Label::new("首页");
    let down = SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(label.on_event(&down), EventResult::NotHandled);
    assert_eq!(label.selected_text(), None);
}

#[test]
fn selectable_label_handles_pointer_down() {
    let mut label = Label::new("首页").selectable();
    let down = SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(label.on_event(&down), EventResult::Handled);
}
