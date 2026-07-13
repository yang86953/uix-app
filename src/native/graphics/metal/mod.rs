//! Metal identity CPU PixelUpload context (macOS).

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
