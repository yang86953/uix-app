//! Vulkan graphics context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::present::GraphicsContextCandidate;

#[path = "adapter/mod.rs"]
pub(crate) mod platform;

pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回平台层已经组装的 context candidate。
) -> Result<GraphicsContextCandidate, Error> {
    platform::create(surface, width, height)
}
