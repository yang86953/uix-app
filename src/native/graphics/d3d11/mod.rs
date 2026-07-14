//! Direct3D 11 graphics context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

pub(crate) mod platform;

pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    platform::create(surface, width, height)
}

#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn create_warp_test_context(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    platform::create_warp_test_context(surface, width, height)
}

#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn warp_test_context_available() -> bool {
    platform::warp_test_context_available()
}
