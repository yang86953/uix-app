use std::collections::BTreeSet;

use crate::native::backends::windows::display::WindowsDisplay;
use crate::native::traits::IDisplay;
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CMONITORS};

#[test]
fn windows_display_enumerates_distinct_monitors_with_primary_first() {
    let display = WindowsDisplay::new();
    let expected_count = unsafe { GetSystemMetrics(SM_CMONITORS) }.max(1);
    let count = display.count();

    assert_eq!(count, expected_count);
    let infos = (0..count)
        .map(|index| display.info(index))
        .collect::<Vec<_>>();
    assert!(
        infos[0].is_primary,
        "primary monitor must have stable index 0"
    );
    assert_eq!(
        infos.iter().filter(|info| info.is_primary).count(),
        1,
        "Windows exposes exactly one primary display"
    );
    assert!(infos.iter().all(|info| {
        info.bounds.w > 0.0
            && info.bounds.h > 0.0
            && info.dpi_scale.is_finite()
            && info.dpi_scale > 0.0
    }));
    let bounds = infos
        .iter()
        .map(|info| {
            (
                info.bounds.x as i32,
                info.bounds.y as i32,
                info.bounds.w as i32,
                info.bounds.h as i32,
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(bounds.len(), infos.len(), "monitor bounds must be distinct");
    assert_eq!(display.info(-1), infos[0]);
    assert_eq!(display.info(count), infos[0]);
}
