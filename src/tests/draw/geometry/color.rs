use crate::tests::common::*;

#[test]
fn rgba_encoding_uses_documented_aarrggbb_channel_order() {
    assert_eq!(
        Color::from_rgba(0x11, 0x22, 0x33, 0x44).to_rgba(),
        0x4411_2233
    );
}

#[test]
fn premultiplied_encoding_keeps_aarrggbb_channel_order() {
    assert_eq!(
        Color::from_rgba(255, 128, 0, 128).premultiplied(),
        0x8080_4000
    );
}

#[test]
fn documented_color_constructors_share_eight_bit_rgba_semantics() {
    assert_eq!(
        Color::rgba(0x11, 0x22, 0x33, 0x44),
        Color::from_rgba(0x11, 0x22, 0x33, 0x44)
    );
    assert_eq!(Color::hex("#1677ff"), Color::from_rgb(0x16, 0x77, 0xff));
    assert_eq!(
        Color::hex("11223344"),
        Color::from_rgba(0x11, 0x22, 0x33, 0x44)
    );
    assert_eq!(Color::hex("not-a-color"), Color::BLACK);
    assert_eq!(Color::black(), Color::BLACK);
    assert_eq!(Color::white(), Color::WHITE);
    assert_eq!(Color::transparent(), Color::TRANSPARENT);
    assert_eq!(Color::red(), Color::RED);
    assert_eq!(Color::green(), Color::GREEN);
    assert_eq!(Color::blue(), Color::BLUE);
    assert_eq!(Color::gray(), Color::from_rgb(128, 128, 128));
    assert_eq!(Color::RED.with_alpha(51), Color::rgba(255, 0, 0, 51));
}
