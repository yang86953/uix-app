//! Direct3D 12 graphics context (planned).

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
use crate::native::traits::present::GraphicsBackend;

/// Placeholder until D3D12 `IGraphicsContext` lands (M6+).
pub fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn crate::native::traits::present::IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        format!(
            "GraphicsBackend {} is planned but not implemented",
            GraphicsBackend::D3d12
        ),
    ))
}
