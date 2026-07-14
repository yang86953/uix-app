//! Platform selection for the Vulkan context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

#[cfg(any(unix, windows))]
mod adapter;

#[cfg(any(unix, windows))]
pub(crate) mod context;

#[cfg(any(unix, windows))]
pub(crate) mod surface;

#[cfg(any(unix, windows))]
pub use context::VulkanContext;

#[cfg(any(unix, windows))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    VulkanContext::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(not(any(unix, windows)))]
#[allow(dead_code)]
pub(crate) fn create(
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
