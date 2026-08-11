//! Direct3D 12 graphics context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::present::GraphicsContextCandidate;
// D3D12 WARP 测试入口仍返回兼容 context trait object。
#[cfg(all(test, feature = "d3d12"))]
use crate::native::present::IGraphicsContext;

pub(crate) mod platform;

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回平台层已经组装的 context candidate。
) -> Result<GraphicsContextCandidate, Error> {
    platform::create(surface, width, height)
}

#[cfg(all(test, feature = "d3d12"))]
pub(crate) fn create_warp_test_context(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    platform::create_warp_test_context(surface, width, height)
}

#[cfg(all(test, feature = "d3d12"))]
pub(crate) fn warp_test_context_available() -> bool {
    platform::warp_test_context_available()
}
