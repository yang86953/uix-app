//! Platform selection for the Direct3D 12 context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::platform::graphics::GpuAdapterInfo;
#[cfg(all(windows, feature = "d3d12"))]
use crate::platform::graphics::GraphicsBackend;
use crate::platform::presentation::{
    GraphicsApi, GraphicsContextCandidate, GraphicsContextCaps, PresentCoherency,
};
// WARP 测试入口返回类型化 GPU context trait object。
#[cfg(all(test, feature = "d3d12"))]
use crate::platform::presentation::GpuRecipeContext;

#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod adapter;
#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod context;
#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod error;
#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod pipeline;
#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod swap_chain;
#[cfg(all(windows, feature = "d3d12"))]
pub(crate) mod transfer;

#[cfg(all(windows, feature = "d3d12"))]
pub use context::D3d12Context;

// 冻结 D3D12 flip-discard swapchain 的真实跨帧保留能力，供候选快照与 Surface 共用。
#[cfg(all(windows, feature = "d3d12"))]
pub(in crate::native::presentation::graphics::d3d12::platform) const D3D12_PRESENT_COHERENCY:
    PresentCoherency = PresentCoherency::FullOnly;

// 组装 D3D12 生产 adapter 的静态 recipe 能力。
#[cfg(all(windows, feature = "d3d12"))]
fn context_caps() -> GraphicsContextCaps {
    // D3D12 当前只承诺完整 swapchain 提交。
    GraphicsContextCaps::gpu_native_swapchain(GraphicsApi::D3d12, D3D12_PRESENT_COHERENCY)
}

#[cfg(all(windows, feature = "d3d12"))]
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    crate::native::presentation::graphics::dxgi::enumerate_adapters(GraphicsBackend::Direct3D12)
}

#[cfg(not(all(windows, feature = "d3d12")))]
pub(crate) fn enumerate_adapters() -> Result<Box<[GpuAdapterInfo]>> {
    use crate::core::Errc;

    Err(Error::new(
        Errc::NotImplemented,
        "Platform::gpu_adapters: Direct3D12 enumeration is unavailable on this target",
    ))
}

#[cfg(all(windows, feature = "d3d12"))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 返回尚待当前 registry row 校验的 adapter 候选记录。
) -> Result<GraphicsContextCandidate, Error> {
    // 创建具体 D3D12 context 后在静态 adapter 边界组装 capability。
    D3d12Context::new(surface, width, height).map(|context| {
        // 从 adapter 创建模块的唯一事实函数组装静态 capability。
        let caps = context_caps();
        // 把 context 与同源快照封装为 candidate。
        GraphicsContextCandidate::gpu(Box::new(context), caps)
    })
}

#[cfg(all(test, windows, feature = "d3d12"))]
pub(crate) fn create_warp_test_context(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn GpuRecipeContext>, Error> {
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
) -> Result<Box<dyn GpuRecipeContext>, Error> {
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
