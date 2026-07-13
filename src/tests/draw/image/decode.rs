use crate::draw::image::decode::*;

const RED_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn decode_png_returns_pixels() {
    let (w, h, pixels) = decode_to_pixels(RED_PNG).expect("decode");
    assert_eq!(w, 1);
    assert_eq!(h, 1);
    assert_eq!(pixels.len(), 1);
    let a = (pixels[0] >> 24) & 0xFF;
    assert!(a > 0, "alpha 应大于 0");
}
