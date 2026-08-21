//! Platform selection for the Vulkan context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::present::{GraphicsApi, GraphicsContextCandidate, GraphicsContextCaps};

#[cfg(any(unix, windows))]
pub(crate) mod adapter;

#[cfg(any(unix, windows))]
pub(crate) mod context;

// Vulkan GPU-native 路径只在 Adapter 内机械映射 platform RHI 契约。
#[allow(dead_code)]
mod rhi;

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

// 组装 Vulkan PixelUpload adapter 的静态 recipe 能力。
#[cfg(any(unix, windows))]
fn context_caps() -> GraphicsContextCaps {
    // Vulkan 当前使用 CPU raster 与专用 swapchain 上传提交。
    GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan)
}

#[cfg(any(unix, windows))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回尚待 registry row 校验的 Vulkan candidate。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建具体 Vulkan context 后在静态 adapter 边界组装 capability。
    VulkanContext::new(surface, width, height).map(|ctx| {
        // 从 adapter 创建模块的唯一事实函数组装静态 capability。
        let caps = context_caps();
        // 把 context 与同源快照封装为 registry candidate。
        GraphicsContextCandidate::pixel_upload(Box::new(ctx), caps)
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
