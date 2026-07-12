//! Platform selection for the Metal identity PixelUpload context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

#[cfg(target_os = "macos")]
mod context;

#[cfg(target_os = "macos")]
pub use context::MetalPixelUploadContext;

#[cfg(target_os = "macos")]
pub(super) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    MetalPixelUploadContext::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub(super) fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend metal is only supported on macOS",
    ))
}
