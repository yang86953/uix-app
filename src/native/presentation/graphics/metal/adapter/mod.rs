//! macOS Metal GPU-native swapchain Adapter。

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::platform::presentation::{GraphicsApi, GraphicsContextCandidate, GraphicsContextCaps};

#[cfg(target_os = "macos")]
pub(crate) mod context;
#[cfg(target_os = "macos")]
mod enumeration;
#[cfg(target_os = "macos")]
mod pipeline;

#[cfg(target_os = "macos")]
pub use context::MetalContext;

#[cfg(target_os = "macos")]
pub(crate) use enumeration::enumerate_adapters;

// 组装 Metal GPU-native swapchain Adapter 的静态 recipe 能力。
#[cfg(target_os = "macos")]
fn context_caps() -> GraphicsContextCaps {
    GraphicsContextCaps::gpu_native_swapchain(
        GraphicsApi::Metal,
        crate::core::PresentCoherency::FullOnly,
    )
}

#[cfg(target_os = "macos")]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回尚待 registry row 校验的 Metal candidate。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建具体 Metal context 后在静态 Adapter 边界组装 capability。
    MetalContext::new(surface, width, height).map(|ctx| {
        // 从 adapter 创建模块的唯一事实函数组装静态 capability。
        let caps = context_caps();
        // 把 context 与同源快照封装为 registry candidate。
        GraphicsContextCandidate::gpu(Box::new(ctx), caps)
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
