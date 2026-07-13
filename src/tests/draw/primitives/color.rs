use crate::tests::common::*;
use std::fmt;

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
