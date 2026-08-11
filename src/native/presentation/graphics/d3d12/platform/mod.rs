//! Platform selection for the Direct3D 12 context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::present::{GraphicsContextCandidate, IGraphicsContext};

#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod adapter;
#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod context;
#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod error;
#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod swap_chain;
#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod transfer;

#[cfg(all(windows, feature = "d3d12"))]
pub use context::D3d12Context;

#[cfg(all(windows, feature = "d3d12"))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回尚待未来 registry row 校验的 adapter 候选记录。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建具体 D3D12 context 后在静态 adapter 边界组装 capability。
    D3d12Context::new(surface, width, height).map(|context| {
        // 从刚创建的具体 adapter 一次读取静态 capability。
        let caps = context.caps();
        // 把 context 与同源快照封装为 candidate。
        GraphicsContextCandidate::new(Box::new(context), caps)
    })
}

#[cfg(all(test, windows, feature = "d3d12"))]
pub(crate) fn create_warp_test_context(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    D3d12Context::new_with_driver(surface, width, height, adapter::D3d12DriverKind::Warp)
        .map(|context| Box::new(context) as _)
}

#[cfg(all(test, windows, feature = "d3d12"))]
pub(crate) const fn warp_test_context_available() -> bool {
    true
}

#[cfg(all(test, not(windows), feature = "d3d12"))]
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

#[cfg(all(test, not(windows), feature = "d3d12"))]
pub(crate) const fn warp_test_context_available() -> bool {
    false
}

#[cfg(not(all(windows, feature = "d3d12")))]
pub(crate) fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
    // 不支持的平台或 feature 保持相同 candidate 返回形状。
) -> Result<GraphicsContextCandidate, Error> {
    use crate::core::Errc;

    let reason = if cfg!(windows) {
        "GraphicsBackend d3d12 requires the d3d12 feature"
    } else {
        "GraphicsBackend d3d12 is only supported on Windows"
    };
    Err(Error::new(Errc::PlatformError, reason))
}
