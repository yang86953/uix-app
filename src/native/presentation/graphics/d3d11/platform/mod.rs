//! Platform selection for the Direct3D 11 context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::present::{GraphicsContextCandidate, IGraphicsContext};

#[cfg(windows)]
pub(crate) mod context;
#[cfg(windows)]
pub(crate) mod pipeline;
#[cfg(windows)]
mod swapchain;

#[cfg(windows)]
pub use context::D3d11Context;

#[cfg(windows)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回尚待 registry row 校验的 adapter 候选记录。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建具体 D3D11 context 后在仍可静态分派的边界组装 capability。
    D3d11Context::new(surface, width, height).map(|ctx| {
        // 从刚创建的具体 adapter 一次读取静态 capability。
        let caps = ctx.caps();
        // 把 context 与同源快照封装为 registry candidate。
        GraphicsContextCandidate::new(Box::new(ctx), caps)
    })
}

// Windows 测试目标保留 D3D11 WARP 平台入口，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, windows, feature = "d3d11"))]
pub(crate) fn create_warp_test_context(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    D3d11Context::new_warp_test_context(surface, width, height).map(|ctx| Box::new(ctx) as _)
}

// Windows 测试目标保留 D3D11 WARP 可用性探测，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, windows, feature = "d3d11"))]
pub(crate) const fn warp_test_context_available() -> bool {
    true
}

// 非 Windows 测试目标保留明确的 D3D11 WARP 不支持入口。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, not(windows), feature = "d3d11"))]
pub(crate) fn create_warp_test_context(
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

// 非 Windows 测试目标保留明确的 D3D11 WARP 不可用性探测。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, not(windows), feature = "d3d11"))]
pub(crate) const fn warp_test_context_available() -> bool {
    false
}

#[cfg(not(windows))]
pub(crate) fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
    // 不支持的平台保持相同 candidate 返回形状。
) -> Result<GraphicsContextCandidate, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend d3d11 is only supported on Windows",
    ))
}
