use crate::draw::rasterizer::image::*;
use crate::tests::common::*;

#[test]
fn empty_destination_is_a_noop() {
    let source = [0xFF_11_22_33; 4];
    let mut pixels = vec![0xDE_AD_BE_EF; 4];
    let clip = Rect::new(0.0, 0.0, 2.0, 2.0);
    let source_rect = Rect::new(0.0, 0.0, 2.0, 2.0);

    blit_image(
        &mut pixels,
        2,
        2,
        clip,
        1.0,
        &source,
        2,
        source_rect,
        Rect::new(0.0, 0.0, 0.0, 2.0),
    );
    blit_image(
        &mut pixels,
        2,
        2,
        clip,
        1.0,
        &source,
        2,
        source_rect,
        Rect::new(0.0, 0.0, 2.0, 0.0),
    );

    assert_eq!(pixels, vec![0xDE_AD_BE_EF; 4]);
}

#[test]
fn image_blit_rejects_undersized_targets_and_extreme_destinations() {
    let source = [0xFF_11_22_33; 4];
    let mut pixels = [0xDE_AD_BE_EF; 3];
    let original = pixels;

    blit_image(
        &mut pixels,
        2,
        2,
        Rect::new(0.0, 0.0, 2.0, 2.0),
        1.0,
        &source,
        2,
        Rect::new(0.0, 0.0, 2.0, 2.0),
        Rect::new(i32::MAX as f32, i32::MIN as f32, i32::MAX as f32, 1.0),
    );

    assert_eq!(pixels, original);
}

#[test]
fn downscaled_image_samples_only_the_visible_destination() {
    let source = [
        0xFF_10_00_00,
        0xFF_20_00_00,
        0xFF_30_00_00,
        0xFF_40_00_00,
        0xFF_50_00_00,
        0xFF_60_00_00,
        0xFF_70_00_00,
        0xFF_80_00_00,
        0xFF_90_00_00,
        0xFF_A0_00_00,
        0xFF_B0_00_00,
        0xFF_C0_00_00,
        0xFF_D0_00_00,
        0xFF_E0_00_00,
        0xFF_F0_00_00,
        0xFF_FF_00_00,
    ];
    let mut pixels = [0u32; 4];

    blit_image(
        &mut pixels,
        2,
        2,
        Rect::new(0.0, 0.0, 2.0, 2.0),
        1.0,
        &source,
        4,
        Rect::new(0.0, 0.0, 4.0, 4.0),
        Rect::new(0.0, 0.0, 2.0, 2.0),
    );

    assert_eq!(pixels, [source[0], source[2], source[8], source[10]]);
}
