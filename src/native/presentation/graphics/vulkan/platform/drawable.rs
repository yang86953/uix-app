//! Vulkan surface 的 logical/drawable extent 适配。

use std::ffi::c_void;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DrawableSize {
    pub(super) logical_width: i32,
    pub(super) logical_height: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

#[cfg(windows)]
pub(super) fn drawable_size(native_surface: *mut c_void, width: i32, height: i32) -> DrawableSize {
    let drawable = crate::native::presentation::graphics::platform::windows::drawable_size(
        native_surface,
        width,
        height,
    );
    DrawableSize {
        logical_width: drawable.logical_width,
        logical_height: drawable.logical_height,
        width: drawable.width,
        height: drawable.height,
    }
}

#[cfg(not(windows))]
pub(super) fn drawable_size(_native_surface: *mut c_void, width: i32, height: i32) -> DrawableSize {
    let width = width.max(1);
    let height = height.max(1);
    DrawableSize {
        logical_width: width,
        logical_height: height,
        width,
        height,
    }
}
