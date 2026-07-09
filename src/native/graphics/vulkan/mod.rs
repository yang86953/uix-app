//! Vulkan graphics context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::traits::present::IGraphicsContext;

mod platform;

pub fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    platform::create(surface, width, height)
}
