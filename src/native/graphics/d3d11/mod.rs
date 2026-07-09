//! Direct3D 11 graphics context.

#[cfg(windows)]
mod context;

#[cfg(windows)]
pub use context::D3d11Context;

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

#[cfg(windows)]
pub fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    D3d11Context::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(not(windows))]
pub fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend d3d11 is only supported on Windows",
    ))
}
