//! Linux backend registry table.

use crate::core::{Errc, Error};
use crate::diagnostics::PendingFailureQueue;
use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::present::{GraphicsApi, IGraphicsContext, PresentMode, RasterMode};
use std::ffi::c_void;

// 原生 OpenGL ES 已接入同一 FramePlan/RHI；Vulkan 仍保留为诊断条目。

#[cfg(feature = "opengles")]
fn create_opengles(
    surface: *mut c_void,
    width: i32,
    height: i32,
    _pending: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    // EGL adapter 直接接管 Wayland surface 与 owner-thread RHI。
    crate::native::presentation::graphics::opengl::create(surface, width, height)
}

#[cfg(not(feature = "opengles"))]
fn create_opengles(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    // feature 关闭时保留 typed disabled 诊断。
    Err(Error::new(
        Errc::PlatformError,
        "graphics feature `opengles` is disabled in this build",
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
const OPENGL_STATUS: BackendStatus = if cfg!(feature = "opengles") {
    BackendStatus::Active
} else {
    BackendStatus::Disabled
};

pub(crate) const PLATFORM_ENTRIES: &[GraphicsBackendEntry] = &[
    GraphicsBackendEntry {
        id: GraphicsApi::Vulkan,
        priority: 20,
        status: VULKAN_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_vulkan,
    },
    GraphicsBackendEntry {
        id: GraphicsApi::OpenGlEs,
        priority: 10,
        status: OPENGL_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_opengles,
    },
];
