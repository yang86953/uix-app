//! Platform selection for OpenGL ES contexts.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::platform::presentation::{
    GraphicsApi, GraphicsContextCandidate, GraphicsContextCaps, PresentCoherency,
};

#[cfg(target_os = "linux")]
pub mod egl;
#[cfg(all(target_os = "linux", uix_gpu_parity_opengl))]
#[path = "../../../../../../tests-src/native/presentation/graphics/opengl/adapter/parity.rs"]
mod parity;
#[cfg(windows)]
pub mod wgl;

#[cfg(target_os = "linux")]
pub use egl::EglContext;
#[cfg(all(target_os = "linux", uix_gpu_parity_opengl))]
pub(crate) use parity::OpenGlWsiParityAdapter;
#[cfg(windows)]
pub use wgl::WglContext;

// 从实际 Surface 事实组装 WGL/EGL 共用的 OpenGL ES 静态 recipe 能力。
fn context_caps(
    // 接收 Surface Adapter 已经声明的呈现一致性。
    present_coherency: PresentCoherency,
) -> GraphicsContextCaps {
    // registry 快照只能复制 Surface 权威事实，不再自行解释平台能力。
    GraphicsContextCaps::gpu_native_swapchain(GraphicsApi::OpenGlEs, present_coherency)
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
        // 从实际 WGL Surface capability 冻结静态 recipe 快照。
        let caps = context_caps(
            // 只复制 Surface 角色拥有的呈现一致性事实。
            crate::platform::presentation::rhi::GraphicsSurface::surface_capabilities(&ctx)
                // registry 只消费选择与门禁需要的字段。
                .present_coherency,
        );
        // 把 context 与同源快照封装为 registry candidate。
        GraphicsContextCandidate::gpu(Box::new(ctx), caps)
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回尚待 registry row 校验的 EGL candidate。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建具体 EGL context 后在静态 adapter 边界组装 capability。
    EglContext::new(surface, width, height).map(|ctx| {
        // 从实际 EGL Surface capability 冻结静态 recipe 快照。
        let caps = context_caps(
            // 只复制 Surface 角色拥有的呈现一致性事实。
            crate::platform::presentation::rhi::GraphicsSurface::surface_capabilities(&ctx)
                // registry 只消费选择与门禁需要的字段。
                .present_coherency,
        );
        // 把 context 与同源快照封装为 registry candidate。
        GraphicsContextCandidate::gpu(Box::new(ctx), caps)
    })
}

#[cfg(not(any(windows, target_os = "linux")))]
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
