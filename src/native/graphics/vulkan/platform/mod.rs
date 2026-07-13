//! Platform selection for the Vulkan context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

#[cfg(any(unix, windows))]
mod context;

#[cfg(any(unix, windows))]
pub use context::VulkanContext;

#[cfg(any(unix, windows))]
pub(super) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    VulkanContext::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(not(any(unix, windows)))]
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

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn linux_vulkan_adapter_compiles_wayland_surface_path() {
        let source = include_str!("context.rs");
        assert!(
            source.contains("create_wayland_surface") && source.contains("wayland_surface"),
            "Linux Vulkan adapter must create a Wayland WSI surface"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_vulkan_adapter_compiles_metal_surface_path() {
        let source = include_str!("context.rs");
        assert!(
            source.contains("create_metal_surface")
                && source.contains("metal_surface")
                && source.contains("CAMetalLayer")
                && source.contains("portability_enumeration")
                && source.contains("portability_subset"),
            "macOS Vulkan adapter must use MoltenVK metal surface + portability"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_vulkan_null_layer_is_typed() {
        let err = super::create(std::ptr::null_mut(), 1, 1).expect_err("null CAMetalLayer");
        assert!(
            err.message().contains("CAMetalLayer")
                || err.message().contains("load Vulkan")
                || err.message().contains("vkCreate"),
            "unexpected macOS Vulkan null-layer error: {}",
            err.message()
        );
    }
}
