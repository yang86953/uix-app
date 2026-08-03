//! Linux backend registry table.

use crate::core::{Errc, Error};
use crate::diagnostics::PendingFailureQueue;
use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::present::{GraphicsBackend, IGraphicsContext, PresentMode, RasterMode};
use std::ffi::c_void;

// 方案 A：wgpu 已移除，原生 Vulkan/OpenGL 后端仍为 test-only，
// Linux 暂无生产 GPU 后端；条目保留以便诊断信息完整。

fn create_opengles(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::NotImplemented,
        "graphics backend `opengles` has no production implementation on Linux",
    ))
}

fn create_vulkan(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::NotImplemented,
        "graphics backend `vulkan` has no production implementation on Linux",
    ))
}

const VULKAN_STATUS: BackendStatus = BackendStatus::Disabled;
const OPENGL_STATUS: BackendStatus = BackendStatus::Disabled;

pub(crate) const PLATFORM_ENTRIES: &[GraphicsBackendEntry] = &[
    GraphicsBackendEntry {
        id: GraphicsBackend::Vulkan,
        priority: 20,
        status: VULKAN_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_vulkan,
    },
    GraphicsBackendEntry {
        id: GraphicsBackend::OpenGlEs,
        priority: 10,
        status: OPENGL_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_opengles,
    },
];
