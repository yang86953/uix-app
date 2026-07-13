use super::*;

#[test]
fn drawable_size_scales_logical_extent_with_monitor_dpi() {
    let size = drawable_size_from_dpi(1200, 800, 192);
    assert_eq!((size.width, size.height), (2400, 1600));
    assert!((size.width as f32 / size.logical_width as f32 - 2.0).abs() < f32::EPSILON);
}
