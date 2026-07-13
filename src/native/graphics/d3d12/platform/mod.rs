//! Platform selection for the Direct3D 12 context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod context;
#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod pipeline;

#[cfg(all(windows, feature = "d3d12"))]
pub use context::D3d12Context;

#[cfg(all(windows, feature = "d3d12"))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    D3d12Context::new(surface, width, height).map(|context| Box::new(context) as _)
}

#[cfg(all(windows, feature = "d3d12"))]
pub(crate) fn create_warp_test_context(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    D3d12Context::new_with_driver(surface, width, height, context::D3d12DriverKind::Warp)
        .map(|context| Box::new(context) as _)
}

#[cfg(all(windows, feature = "d3d12"))]
pub(crate) const fn warp_test_context_available() -> bool {
    true
}

#[cfg(all(not(windows), feature = "d3d12"))]
pub(crate) fn create_warp_test_context(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "D3D12 WARP tests are only supported on Windows",
    ))
}

#[cfg(all(not(windows), feature = "d3d12"))]
pub(crate) const fn warp_test_context_available() -> bool {
    false
}

#[cfg(not(all(windows, feature = "d3d12")))]
pub(crate) fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    use crate::core::Errc;

    let reason = if cfg!(windows) {
        "GraphicsBackend d3d12 requires the d3d12 feature"
    } else {
        "GraphicsBackend d3d12 is only supported on Windows"
    };
    Err(Error::new(Errc::PlatformError, reason))
}

