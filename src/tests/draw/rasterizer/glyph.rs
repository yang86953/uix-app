use crate::draw::rasterizer::glyph::blit_glyph;
use crate::tests::common::*;

#[test]
fn malformed_or_overflowing_glyph_buffers_are_noops() {
    let mut pixels = [0xDEAD_BEEF; 4];
    let original = pixels;
    let clip = Rect::new(0.0, 0.0, 2.0, 2.0);

    blit_glyph(
        &mut pixels,
        2,
        2,
        clip,
        1.0,
        0,
        0,
        &[255],
        2,
        2,
        Color::white(),
    );
    blit_glyph(
        &mut pixels,
        2,
        2,
        clip,
        1.0,
        i32::MAX,
        i32::MIN,
        &[255],
        usize::MAX,
        2,
        Color::white(),
    );

    assert_eq!(pixels, original);
}

#[test]
fn glyph_blit_only_visits_the_visible_destination() {
    let mut pixels = [0u32; 6];
    let coverage = [0, 255, 255, 0, 255, 255];

    blit_glyph(
        &mut pixels,
        3,
        2,
        Rect::new(0.0, 0.0, 3.0, 2.0),
        1.0,
        -1,
        0,
        &coverage,
        3,
        2,
        Color::white(),
    );

    assert_ne!(pixels[0], 0);
    assert_ne!(pixels[1], 0);
    assert_eq!(pixels[2], 0);
    assert_ne!(pixels[3], 0);
    assert_ne!(pixels[4], 0);
    assert_eq!(pixels[5], 0);
}
