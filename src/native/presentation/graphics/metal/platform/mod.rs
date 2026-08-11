//! Platform selection for the Metal identity PixelUpload context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::present::{GraphicsApi, GraphicsContextCandidate, GraphicsContextCaps};

#[cfg(target_os = "macos")]
pub(crate) mod context;

#[cfg(target_os = "macos")]
pub use context::MetalPixelUploadContext;

// 组装 Metal identity PixelUpload adapter 的静态 recipe 能力。
#[cfg(target_os = "macos")]
fn context_caps() -> GraphicsContextCaps {
    // Metal 当前作为 CPU retained pixels 的专用上传提交后端。
    GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Metal)
}

#[cfg(target_os = "macos")]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回尚待 registry row 校验的 Metal candidate。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建具体 Metal context 后在静态 adapter 边界组装 capability。
    MetalPixelUploadContext::new(surface, width, height).map(|ctx| {
        // 从 adapter 创建模块的唯一事实函数组装静态 capability。
        let caps = context_caps();
        // 把 context 与同源快照封装为 registry candidate。
        GraphicsContextCandidate::pixel_upload(Box::new(ctx), caps)
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
