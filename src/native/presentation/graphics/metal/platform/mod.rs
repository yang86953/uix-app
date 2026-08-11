//! Platform selection for the Metal identity PixelUpload context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::present::{GraphicsContextCandidate, IGraphicsContext};

#[cfg(target_os = "macos")]
pub(crate) mod context;

#[cfg(target_os = "macos")]
pub use context::MetalPixelUploadContext;

#[cfg(target_os = "macos")]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回尚待 registry row 校验的 Metal candidate。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建具体 Metal context 后在静态 adapter 边界组装 capability。
    MetalPixelUploadContext::new(surface, width, height).map(|ctx| {
        // 从刚创建的具体 adapter 一次读取静态 capability。
        let caps = ctx.caps();
        // 把 context 与同源快照封装为 registry candidate。
        GraphicsContextCandidate::new(Box::new(ctx), caps)
    })
}

#[cfg(not(target_os = "macos"))]
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
        "GraphicsBackend metal is only supported on macOS",
    ))
}
