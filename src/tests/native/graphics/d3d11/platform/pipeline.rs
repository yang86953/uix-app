use super::visible_pixel_bounds;

#[test]
fn soft_upload_bounds_is_tight_and_ignores_transparent_pixels() {
    let mut pixels = vec![0u32; 8 * 6];
    pixels[8 + 2] = 0xFF00_0001;
    pixels[3 * 8 + 5] = 0x8000_0002;
    pixels[5 * 8 + 7] = 0x0000_00FF;

    assert_eq!(visible_pixel_bounds(&pixels, 8, 6), Some((2, 1, 4, 3)));
    assert_eq!(visible_pixel_bounds(&[0; 4], 2, 2), None);
}
