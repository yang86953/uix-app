//! Platform selection for OpenGL ES contexts.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::present::{
    GraphicsApi, GraphicsContextCandidate, GraphicsContextCaps, PresentCoherency,
};

/// The damage extension alone does not prove buffer preservation or buffer-age
/// semantics, so EGL must not advertise partial present yet.
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) const EGL_PARTIAL_PRESENT: bool = false;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod egl;
#[cfg(windows)]
pub mod wgl;

#[cfg(all(unix, not(target_os = "macos")))]
pub use egl::EglContext;
#[cfg(windows)]
pub use wgl::WglContext;

// 组装 WGL/EGL 共用的 OpenGL ES 静态 recipe 能力。
fn context_caps() -> GraphicsContextCaps {
    // 两个平台当前都只承诺完整 swapchain 提交。
    GraphicsContextCaps::gpu_native_swapchain(GraphicsApi::OpenGlEs, PresentCoherency::FullOnly)
}

#[cfg(windows)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回尚待 registry row 校验的 WGL candidate。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建具体 WGL context 后在静态 adapter 边界组装 capability。
    WglContext::new(surface, width, height).map(|ctx| {
        // 从 adapter 创建模块的唯一事实函数组装静态 capability。
        let caps = context_caps();
        // 把 context 与同源快照封装为 registry candidate。
        GraphicsContextCandidate::gpu(Box::new(ctx), caps)
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回尚待 registry row 校验的 EGL candidate。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建具体 EGL context 后在静态 adapter 边界组装 capability。
    EglContext::new(surface, width, height).map(|ctx| {
        // 从 adapter 创建模块的唯一事实函数组装静态 capability。
        let caps = context_caps();
        // 把 context 与同源快照封装为 registry candidate。
        GraphicsContextCandidate::gpu(Box::new(ctx), caps)
    })
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub(crate) fn create(
    _surface: *mut c_void,
    _width: i32,
    _height: i32,
    // 不支持的平台保持相同 candidate 返回形状。
) -> Result<GraphicsContextCandidate, Error> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend opengles is not supported on this platform",
    ))
}
