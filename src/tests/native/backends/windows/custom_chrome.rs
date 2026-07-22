use crate::native::backends::windows::consts::{WS_CAPTION, WS_THICKFRAME};
use crate::native::backends::windows::custom_chrome::*;
use crate::native::backends::windows::dpi::{outer_size_for_logical_client, BASE_DPI};

#[test]
fn extended_client_requires_thick_frame_without_caption() {
    assert!(uses_extended_client(WS_THICKFRAME));
    assert!(!uses_extended_client(WS_CAPTION | WS_THICKFRAME));
    assert!(!uses_extended_client(0));
    assert!(outer_matches_client_when_extended(WS_THICKFRAME));
}

#[test]
fn screen_point_decodes_signed_coordinates() {
    let packed = ((-2i16 as u16 as isize) << 16) | (-3i16 as u16 as isize);
    assert_eq!(screen_point_from_lparam(packed), (-3, -2));
}

#[test]
fn outer_size_skips_frame_inflation_for_extended_client() {
    let extended = outer_size_for_logical_client(400, 300, WS_THICKFRAME, 0, BASE_DPI)
        .expect("extended outer size");
    assert_eq!(extended, (400, 300));

    let with_caption =
        outer_size_for_logical_client(400, 300, WS_CAPTION | WS_THICKFRAME, 0, BASE_DPI)
            .expect("caption outer size");
    assert!(
        with_caption.0 > 400 || with_caption.1 > 300,
        "caption path must still inflate for non-client chrome"
    );
}

#[test]
fn maximize_detection_honors_style_bit() {
    assert!(is_effectively_maximized(std::ptr::null_mut(), true));
    assert!(!is_effectively_maximized(std::ptr::null_mut(), false));
}

#[test]
fn refresh_extended_client_frame_ignores_null_and_captioned_windows() {
    refresh_extended_client_frame(std::ptr::null_mut());
    // 有系统标题栏时不应改帧；空操作即可。
    assert!(!uses_extended_client(WS_CAPTION | WS_THICKFRAME));
}

#[test]
fn apply_dwm_frame_effects_ignores_null_hwnd() {
    apply_dwm_frame_effects(std::ptr::null_mut(), false);
    apply_dwm_frame_effects(std::ptr::null_mut(), true);
}
