//! Platform selection for the Vulkan context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

#[cfg(all(unix, not(target_os = "macos")))]
mod context;

#[cfg(all(unix, not(target_os = "macos")))]
pub use context::VulkanContext;

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    VulkanContext::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(windows)]
#[allow(dead_code)]
pub(super) fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend vulkan WSI adapter is not implemented on Windows",
    ))
}

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
        "GraphicsBackend vulkan portability adapter is not implemented on macOS",
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
