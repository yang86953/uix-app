//! Platform selection for the Direct3D 11 context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::native::present::{GraphicsApi, GraphicsContextCandidate, GraphicsContextCaps};
// WARP 测试入口返回类型化 GPU context trait object。
#[cfg(all(test, feature = "d3d11"))]
use crate::native::present::GpuRecipeContext;

#[cfg(windows)]
pub(crate) mod context;
#[cfg(windows)]
pub(crate) mod pipeline;
#[cfg(windows)]
mod swapchain;

#[cfg(windows)]
// D3D11 上下文只在 crate 内部图形组合边界重导出。
pub(crate) use context::D3d11Context;

// 显式 parity feature 保持测试入口在 D3D11 platform Adapter 内。
#[cfg(all(windows, feature = "d3d11-parity-test"))]
pub(crate) fn run_gpu_parity_test() {
    context::run_gpu_parity_test();
}

// 从实际创建的 DXGI swapchain 事实组装 D3D11 静态 recipe 能力。
#[cfg(windows)]
fn context_caps(
    present_coherency: crate::native::present::PresentCoherency,
) -> GraphicsContextCaps {
    // 返回由实际 swapchain 一致性证明构成的 adapter 快照。
    GraphicsContextCaps::gpu_native_swapchain(GraphicsApi::D3d11, present_coherency)
}

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
        // 从实际 Surface capability 冻结 registry 需要的构造期 recipe 快照。
        let caps = context_caps(
            // 只读取 Surface 角色拥有的同源 swapchain 保留证明。
            crate::platform::presentation::rhi::GraphicsSurface::surface_capabilities(&ctx)
                // recipe 只复制选择与门禁所需的静态字段。
                .present_coherency,
        );
        // 把 context 与同源快照封装为 registry candidate。
        GraphicsContextCandidate::gpu(Box::new(ctx), caps)
    })
}

// Windows 测试目标保留 D3D11 WARP 平台入口，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, windows, feature = "d3d11"))]
pub(crate) fn create_warp_test_context(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn GpuRecipeContext>, Error> {
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
) -> Result<Box<dyn GpuRecipeContext>, Error> {
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
