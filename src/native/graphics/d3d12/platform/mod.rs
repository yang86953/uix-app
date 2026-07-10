//! Platform selection for the Direct3D 12 context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

#[cfg(all(windows, feature = "d3d12"))]
mod context;

#[cfg(all(windows, feature = "d3d12"))]
pub use context::D3d12Context;

#[cfg(all(windows, feature = "d3d12"))]
pub(super) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    D3d12Context::new(surface, width, height).map(|context| Box::new(context) as _)
}

#[cfg(not(all(windows, feature = "d3d12")))]
pub(super) fn create(
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

#[cfg(all(test, windows, not(feature = "d3d12")))]
mod tests {
    #[test]
    fn disabled_feature_cannot_create_a_real_context() {
        let error = match super::create(std::ptr::null_mut(), 64, 64) {
            Ok(_) => panic!("disabled D3D12 feature must not create a context"),
            Err(error) => error,
        };
        assert!(error.what().contains("requires the d3d12 feature"));
    }
}
