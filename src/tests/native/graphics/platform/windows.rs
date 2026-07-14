use crate::native::graphics::platform::windows::*;

#[test]
fn drawable_size_scales_logical_extent_with_monitor_dpi() {
    let size = drawable_size_from_dpi(1200, 800, 192);
    assert_eq!((size.width, size.height), (2400, 1600));
    assert!((size.width as f32 / size.logical_width as f32 - 2.0).abs() < f32::EPSILON);
}

#[test]
fn drawable_size_keeps_physical_client_pixels_and_derives_logical_extent() {
    let size = drawable_size_from_client_pixels(2400, 1600, 192);
    assert_eq!((size.logical_width, size.logical_height), (1200, 800));
    assert_eq!((size.width, size.height), (2400, 1600));
}
