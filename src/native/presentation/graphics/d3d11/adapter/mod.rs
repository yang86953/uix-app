//! Platform selection for the Direct3D 11 context.

use std::ffi::c_void;

use crate::core::{Error, Result};
use crate::platform::presentation::{GraphicsApi, GraphicsContextCandidate, GraphicsContextCaps};
// WARP 测试入口返回类型化 GPU context trait object。

#[cfg(windows)]
pub(crate) mod context;
mod enumeration;
#[cfg(windows)]
pub(crate) mod pipeline;
#[cfg(windows)]
mod swapchain;

#[cfg(windows)]
// D3D11 上下文只在 crate 内部图形组合边界重导出。
pub(crate) use context::D3d11Context;
pub(crate) use enumeration::enumerate_adapters;

// 显式 parity feature 保持测试入口在 D3D11 platform Adapter 内。
#[cfg(all(windows, uix_gpu_parity_d3d11))]
pub(crate) fn run_gpu_parity_test() {
    context::run_gpu_parity_test();
}

// 从实际创建的 DXGI swapchain 事实组装 D3D11 静态 recipe 能力。
#[cfg(windows)]
fn context_caps(
    present_coherency: crate::platform::presentation::PresentCoherency,
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

// 仅测试构建的 WARP 辅助位于 tests-src，经模块级 include! 保持原作用域。
#[cfg(test)]
include!("../../../../../../tests-src/native/presentation/graphics/d3d11/adapter/warp_test_fns.rs");
