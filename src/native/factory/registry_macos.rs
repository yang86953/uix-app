//! macOS backend registry table.

use crate::core::{Errc, Error};
use crate::diagnostics::PendingFailureQueue;
use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::present::{GraphicsApi, GraphicsContextCandidate, PresentMode, RasterMode};
use std::ffi::c_void;

// 方案 A：wgpu 已移除，原生 Metal/Vulkan 后端仍为 test-only，
// macOS 暂无生产 GPU 后端；条目保留以便诊断信息完整。

fn create_metal(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<GraphicsContextCandidate, Error> {
    Err(Error::new(
        Errc::NotImplemented,
        "graphics backend `metal` has no production implementation on macOS",
    ))
}

fn create_vulkan(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<GraphicsContextCandidate, Error> {
    Err(Error::new(
        Errc::NotImplemented,
        "graphics backend `vulkan` has no production implementation on macOS",
    ))
}

const METAL_STATUS: BackendStatus = BackendStatus::Disabled;
const VULKAN_STATUS: BackendStatus = BackendStatus::Disabled;

pub(crate) const PLATFORM_ENTRIES: &[GraphicsBackendEntry] = &[
    GraphicsBackendEntry {
        id: GraphicsApi::Metal,
        priority: 20,
        status: METAL_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_metal,
    },
    GraphicsBackendEntry {
        id: GraphicsApi::Vulkan,
        priority: 10,
        status: VULKAN_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_vulkan,
    },
];
