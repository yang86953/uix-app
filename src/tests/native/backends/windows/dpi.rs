use crate::native::backends::windows::dpi::{
    logical_extent_to_physical, physical_extent_to_logical, physical_point_to_logical, valid_dpi,
    BASE_DPI,
};

#[test]
fn extent_conversion_round_trips_common_monitor_scales() {
    for dpi in [96, 120, 144, 168, 192, 240, 288] {
        for logical in [1, 2, 17, 320, 801, 1920] {
            let physical = logical_extent_to_physical(logical, dpi);
            let restored = physical_extent_to_logical(physical, dpi);
            assert!(
                (restored - logical).abs() <= 1,
                "dpi={dpi} logical={logical}"
            );
        }
    }
}

#[test]
fn invalid_dpi_and_non_positive_extents_are_bounded() {
    assert_eq!(valid_dpi(0), BASE_DPI);
    assert_eq!(logical_extent_to_physical(25, 0), 25);
    assert_eq!(physical_extent_to_logical(25, 0), 25);
    assert_eq!(logical_extent_to_physical(0, 192), 0);
    assert_eq!(physical_extent_to_logical(-1, 192), 0);
}

#[test]
fn physical_pointer_coordinates_keep_fractional_logical_position() {
    assert!((physical_point_to_logical(151, 144) - 100.666_664).abs() < 0.000_1);
}
