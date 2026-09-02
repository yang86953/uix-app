//! Linux backend registry table.

use crate::core::{Errc, Error};
use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::platform::presentation::{
    GraphicsApi, GraphicsContextCandidate, PresentMode, RasterMode,
};
use std::ffi::c_void;

// Vulkan GPU-native swapchain 是优先生产路径；OpenGL ES 只保留兼容候选。

#[cfg(feature = "opengles")]
fn create_opengles(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<GraphicsContextCandidate, Error> {
    // EGL adapter 直接接管 Wayland surface 与 owner-thread RHI。
    crate::native::presentation::graphics::opengl::create(surface, width, height)
}

#[cfg(not(feature = "opengles"))]
fn create_opengles(
    _: *mut c_void,
    _: i32,
    _: i32,
) -> Result<GraphicsContextCandidate, Error> {
    // feature 关闭时保留 typed disabled 诊断。
    Err(Error::new(
        Errc::PlatformError,
        "graphics feature `opengles` is disabled in this build",
    ))
}

#[cfg(feature = "vulkan")]
fn create_vulkan(
    surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<GraphicsContextCandidate, Error> {
    // Vulkan Adapter 只处理 Wayland/Vulkan WSI，像素语义由共享 Drawing 路径产生。
    crate::native::presentation::graphics::vulkan::create(surface, width, height)
}

#[cfg(not(feature = "vulkan"))]
fn create_vulkan(
    _: *mut c_void,
    _: i32,
    _: i32,
) -> Result<GraphicsContextCandidate, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "graphics feature `vulkan` is disabled in this build",
    ))
}

const VULKAN_STATUS: BackendStatus = if cfg!(feature = "vulkan") {
    BackendStatus::Active
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
        id: GraphicsApi::Vulkan,
        priority: 100,
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
