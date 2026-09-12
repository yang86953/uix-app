//! Platform selection for the Vulkan context.

use std::ffi::c_void;

use crate::core::{Error, PresentCoherency, Result};
use crate::platform::presentation::{GraphicsApi, GraphicsContextCandidate, GraphicsContextCaps};

#[cfg(any(unix, windows))]
pub(crate) mod adapter;

#[cfg(any(unix, windows))]
mod enumeration;

#[cfg(any(unix, windows))]
pub(crate) use enumeration::enumerate_adapters;

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

#[cfg(uix_gpu_parity_vulkan)]
#[path = "../../../../../../tests-src/native/presentation/graphics/vulkan/adapter/parity.rs"]
mod parity;

#[cfg(any(unix, windows))]
pub use context::VulkanContext;
// 显式 parity 根只选择中立 Adapter 实现，不取得具体故障值。
#[cfg(uix_gpu_parity_vulkan)]
pub(crate) use parity::{VulkanHeadlessUiParityAdapter, VulkanWsiParityAdapter};

// 组装 Vulkan GPU-native swapchain adapter 的静态 recipe 能力。
#[cfg(any(unix, windows))]
fn context_caps() -> GraphicsContextCaps {
    // Vulkan swapchain 不承诺跨 image 像素保留，因此固定要求完整帧。
    GraphicsContextCaps::gpu_native_swapchain(GraphicsApi::Vulkan, PresentCoherency::FullOnly)
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
        // 把同一 owner 的 GraphicsDevice + GraphicsSurface 作为 GPU-native candidate。
        GraphicsContextCandidate::gpu(Box::new(ctx), caps)
    })
}

// 显式测试 feature 才允许 crate 根调用真实 GPU 离屏 parity harness。
#[cfg(uix_gpu_parity_vulkan)]
pub(crate) fn run_gpu_parity_test() {
    context::run_gpu_parity_test();
}

// 显式测试 feature 执行不创建原生对象的连续帧同步契约。
#[cfg(uix_gpu_parity_vulkan)]
pub(crate) fn run_present_completion_contract_test() {
    context::run_present_completion_contract_test();
}

// 显式测试 feature 验证真实共享 device 与每窗口恢复边界。
#[cfg(uix_gpu_parity_vulkan)]
pub(crate) fn run_shared_device_contract_test() {
    device::run_shared_device_contract_test();
    crate::draw::renderer::recovery_driver::run_multi_window_device_loss_contract_test();
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
