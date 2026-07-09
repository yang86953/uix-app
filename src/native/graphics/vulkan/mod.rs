//! Vulkan graphics context.

#[cfg(all(unix, not(target_os = "macos")))]
mod context;

#[cfg(all(unix, not(target_os = "macos")))]
pub use context::VulkanContext;

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
use crate::native::traits::present::IGraphicsContext;

#[cfg(all(unix, not(target_os = "macos")))]
pub fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    VulkanContext::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend vulkan is only supported on Linux Wayland",
    ))
}
