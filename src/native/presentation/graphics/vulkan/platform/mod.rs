//! Platform selection for the Vulkan context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::present::{GraphicsContextCandidate, IGraphicsContext};

#[cfg(any(unix, windows))]
pub(crate) mod adapter;

#[cfg(any(unix, windows))]
pub(crate) mod context;

#[cfg(any(unix, windows))]
mod device;

#[cfg(any(unix, windows))]
pub(crate) mod fault;

#[cfg(any(unix, windows))]
mod drawable;

#[cfg(any(unix, windows))]
pub(crate) mod surface;

#[cfg(any(unix, windows))]
pub use context::VulkanContext;

#[cfg(any(unix, windows))]
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回尚待 registry row 校验的 Vulkan candidate。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建具体 Vulkan context 后在静态 adapter 边界组装 capability。
    VulkanContext::new(surface, width, height).map(|ctx| {
        // 从刚创建的具体 adapter 一次读取静态 capability。
        let caps = ctx.caps();
        // 把 context 与同源快照封装为 registry candidate。
        GraphicsContextCandidate::new(Box::new(ctx), caps)
    })
}

#[cfg(not(any(unix, windows)))]
#[allow(dead_code)]
pub(crate) fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
    // 不支持的平台保持相同 candidate 返回形状。
) -> Result<GraphicsContextCandidate, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend vulkan is not supported on this platform",
    ))
}
