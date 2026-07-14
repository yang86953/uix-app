use crate::draw::Color;
use crate::ui::style::ColorValue;
use crate::ui::view::{label, StyleExt};

#[test]
fn style_ext_records_all_interaction_backgrounds() {
    let node = StyleExt::bg_focus(label("stateful"), Color::from_rgb(255, 215, 0))
        .bg(Color::red())
        .bg_hover(Color::green())
        .bg_active(Color::blue());

    assert_eq!(
        node.style.background,
        Some(ColorValue::Custom(Color::red()))
    );
    assert_eq!(
        node.style.background_hover,
        Some(ColorValue::Custom(Color::green()))
    );
    assert_eq!(
        node.style.background_focus,
        Some(ColorValue::Custom(Color::from_rgb(255, 215, 0)))
    );
    assert_eq!(
        node.style.background_active,
        Some(ColorValue::Custom(Color::blue()))
    );
}
