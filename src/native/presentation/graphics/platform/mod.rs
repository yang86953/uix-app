//! Shared platform surface descriptors for graphics API modules.

use crate::native::windowing::window::PlatformWindow;

#[cfg(windows)]
pub mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux;

/// Returns the logical client extent observed from the native window.
///
/// Platform-specific drawable probing belongs to the native graphics boundary;
/// callers in the application loop should only consume the normalized extent.
pub(crate) fn native_client_logical_extent(platform_window: &dyn PlatformWindow) -> (i32, i32) {
    let cached_width = platform_window.properties().width();
    let cached_height = platform_window.properties().height();
    #[cfg(windows)]
    {
        let hwnd = platform_window.native_handle().native_window();
        if !hwnd.is_null() {
            let drawable = windows::drawable_size(hwnd, cached_width, cached_height);
            return (drawable.logical_width, drawable.logical_height);
        }
    }
    (cached_width, cached_height)
}

