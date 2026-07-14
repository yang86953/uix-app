use crate::native::backends::windows::dpi::{
    logical_extent_to_physical, physical_extent_to_logical, physical_point_to_logical, valid_dpi,
    with_per_monitor_v2, BASE_DPI,
};
use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, GetThreadDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
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

#[test]
fn per_monitor_scope_is_bounded_and_restores_the_calling_thread() {
    let before = unsafe { GetThreadDpiAwarenessContext() };
    with_per_monitor_v2(|| {
        let current = unsafe { GetThreadDpiAwarenessContext() };
        assert!(bool::from(unsafe {
            AreDpiAwarenessContextsEqual(current, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
        }));
        Ok(())
    })
    .expect("bounded Per-Monitor V2 scope");
    let after = unsafe { GetThreadDpiAwarenessContext() };
    assert!(bool::from(unsafe {
        AreDpiAwarenessContextsEqual(before, after)
    }));
}
