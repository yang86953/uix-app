//! Platform selection for the Vulkan context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

#[cfg(any(all(unix, not(target_os = "macos")), windows))]
mod context;

#[cfg(any(all(unix, not(target_os = "macos")), windows))]
pub use context::VulkanContext;

#[cfg(any(all(unix, not(target_os = "macos")), windows))]
pub(super) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    VulkanContext::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

/// macOS Vulkan remains Planned: needs MoltenVK + a Metal-compatible layer
/// (`CAMetalLayer`), while the current window surface exposes `CALayer`.
/// Keep a typed stub so Auto probe skips and explicit requests fail honestly.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub(super) fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend vulkan portability adapter is not implemented on macOS (requires MoltenVK + CAMetalLayer)",
    ))
}

#[cfg(not(any(
    all(unix, not(target_os = "macos")),
    windows,
    target_os = "macos"
)))]
#[allow(dead_code)]
pub(super) fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend vulkan is not supported on this platform",
    ))
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn windows_vulkan_adapter_compiles_win32_surface_path() {
        let source = include_str!("context.rs");
        assert!(
            source.contains("create_win32_surface") && source.contains("win32_surface"),
            "Windows Vulkan adapter must create a Win32 WSI surface"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_vulkan_stub_names_moltenvk_gap() {
        let err = super::create(std::ptr::null_mut(), 1, 1).expect_err("macOS stub");
        assert!(err.message().contains("MoltenVK"));
        assert!(err.message().contains("CAMetalLayer"));
    }
}
