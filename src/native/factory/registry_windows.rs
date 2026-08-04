//! Windows backend registry table.

use crate::core::{Errc, Error};
use crate::diagnostics::PendingFailureQueue;
use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::present::{GraphicsBackend, IGraphicsContext, PresentMode, RasterMode};
use std::ffi::c_void;

// 生产 GPU 后端：原生 D3D11（wgpu 已移除）。
#[cfg(feature = "d3d11")]
fn create_d3d11(
    surface: *mut c_void,
    width: i32,
    height: i32,
    _pending: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    crate::native::presentation::graphics::d3d11::create(surface, width, height)
}

#[cfg(not(feature = "d3d11"))]
fn create_d3d11(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(feature_disabled("d3d11"))
}

#[cfg(feature = "vulkan")]
fn create_vulkan(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    // 方案 A：原生 Vulkan 后端仍为 test-only，未注册生产入口。
    Err(Error::new(
        Errc::NotImplemented,
        "graphics backend `vulkan` has no production implementation on Windows",
    ))
}

#[cfg(not(feature = "vulkan"))]
fn create_vulkan(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(feature_disabled("vulkan"))
}

#[cfg(feature = "opengles")]
fn create_opengles(
    surface: *mut c_void,
    width: i32,
    height: i32,
    _pending: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    // OpenGL ES 已接入同一 FramePlan/RHI，进入生产 registry 的次级候选。
    crate::native::presentation::graphics::opengl::create(surface, width, height)
}

#[cfg(not(feature = "opengles"))]
fn create_opengles(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(feature_disabled("opengles"))
}

#[allow(dead_code)] // d3d11 生产构建下无 disabled 行引用它
fn feature_disabled(feature: &str) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("graphics feature `{feature}` is disabled in this build"),
    )
}

const D3D11_STATUS: BackendStatus = if cfg!(feature = "d3d11") {
    BackendStatus::Active
} else {
    BackendStatus::Disabled
};

const VULKAN_STATUS: BackendStatus = if cfg!(feature = "vulkan") {
    BackendStatus::Disabled
} else {
    BackendStatus::Disabled
};

const OPENGL_STATUS: BackendStatus = if cfg!(feature = "opengles") {
    BackendStatus::Active
} else {
    BackendStatus::Disabled
};

pub(crate) const PLATFORM_ENTRIES: &[GraphicsBackendEntry] = &[
    GraphicsBackendEntry {
        id: GraphicsBackend::D3d11,
        priority: 30,
        status: D3D11_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_d3d11,
    },
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
