//! Platform selection for the Direct3D 11 context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

#[cfg(windows)]
mod context;
#[cfg(windows)]
mod pipeline;

#[cfg(windows)]
pub use context::D3d11Context;

#[cfg(windows)]
pub(super) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    D3d11Context::new(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(all(test, windows, feature = "d3d11"))]
pub(super) fn create_warp_test_context(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    D3d11Context::new_warp_test_context(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

#[cfg(all(test, windows, feature = "d3d11"))]
pub(super) const fn warp_test_context_available() -> bool {
    true
}

#[cfg(all(test, not(windows), feature = "d3d11"))]
pub(super) fn create_warp_test_context(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "D3D11 WARP tests are only supported on Windows",
    ))
}

#[cfg(all(test, not(windows), feature = "d3d11"))]
pub(super) const fn warp_test_context_available() -> bool {
    false
}

#[cfg(not(windows))]
pub(super) fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend d3d11 is only supported on Windows",
    ))
}
