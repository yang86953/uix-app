use crate::draw::rasterizer::core::{fill_rect_raw, put_pixel, put_pixel_aa};

#[test]
fn pixel_writes_reject_negative_and_overflowing_indices() {
    let mut pixels = [0xDEAD_BEEF; 4];
    let original = pixels;

    put_pixel(
        &mut pixels,
        i32::MAX,
        i32::MAX - 1,
        i32::MAX - 1,
        0,
        0,
        i32::MAX,
        i32::MAX,
        0xFFFF_FFFF,
    );
    put_pixel_aa(
        &mut pixels,
        2,
        -1,
        -1,
        i32::MIN,
        i32::MIN,
        i32::MAX,
        i32::MAX,
        0xFFFF_FFFF,
        0.5,
    );

    assert_eq!(pixels, original);
}

#[test]
fn fill_rect_clips_before_iteration_and_handles_extreme_extents() {
    let mut pixels = [0u32; 6];
    fill_rect_raw(
        &mut pixels,
        3,
        -1,
        0,
        i32::MAX,
        i32::MAX,
        0,
        0,
        3,
        2,
        0xFFFF_FFFF,
    );

    assert_eq!(pixels, [0xFFFF_FFFF; 6]);
}
