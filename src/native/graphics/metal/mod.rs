//! Metal graphics context (macOS).

use std::ffi::c_void;

#[cfg(target_os = "macos")]
mod context;

#[cfg(target_os = "macos")]
pub use context::MetalContext;

#[cfg(not(target_os = "macos"))]
use crate::core::{Errc, Error, Result};

#[cfg(not(target_os = "macos"))]
pub fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn crate::native::traits::present::IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend metal is only supported on macOS",
    ))
}

#[cfg(target_os = "macos")]
pub fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn crate::native::traits::present::IGraphicsContext>, Error> {
    MetalContext::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}
