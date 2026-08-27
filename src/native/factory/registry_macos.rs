//! macOS backend registry table.

use crate::core::{Errc, Error};
use crate::diagnostics::PendingFailureQueue;
use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::platform::presentation::{
    GraphicsApi, GraphicsContextCandidate, PresentMode, RasterMode,
};
use std::ffi::c_void;

// Vulkan GPU-native swapchain 是优先生产路径；Metal 提供原生次级候选。
#[cfg(feature = "metal")]
fn create_metal(
    surface: *mut c_void,
    width: i32,
    height: i32,
    _pending: PendingFailureQueue,
) -> Result<GraphicsContextCandidate, Error> {
    crate::native::presentation::graphics::metal::platform::create(surface, width, height)
}

#[cfg(not(feature = "metal"))]
fn create_metal(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<GraphicsContextCandidate, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "graphics feature `metal` is disabled in this build",
    ))
}

#[cfg(feature = "vulkan")]
fn create_vulkan(
    surface: *mut c_void,
    width: i32,
    height: i32,
    _pending: PendingFailureQueue,
) -> Result<GraphicsContextCandidate, Error> {
    // MoltenVK surface 只负责原生 WSI，Drawing 仍复用同一 FramePlan 语义。
    crate::native::presentation::graphics::vulkan::create(surface, width, height)
}

#[cfg(not(feature = "vulkan"))]
fn create_vulkan(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<GraphicsContextCandidate, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "graphics feature `vulkan` is disabled in this build",
    ))
}

const METAL_STATUS: BackendStatus = if cfg!(feature = "metal") {
    BackendStatus::Active
} else {
    BackendStatus::Disabled
};
const VULKAN_STATUS: BackendStatus = if cfg!(feature = "vulkan") {
    BackendStatus::Active
} else {
    BackendStatus::Disabled
};

pub(crate) const PLATFORM_ENTRIES: &[GraphicsBackendEntry] = &[
    GraphicsBackendEntry {
        id: GraphicsApi::Metal,
        priority: 90,
        status: METAL_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_metal,
    },
    GraphicsBackendEntry {
        id: GraphicsApi::Vulkan,
        priority: 100,
        status: VULKAN_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_vulkan,
    },
];
