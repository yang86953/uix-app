//! macOS backend registry table.

use crate::diagnostics::PendingFailureQueue;
use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::present::{GraphicsBackend, PresentMode, RasterMode};

#[cfg(any(not(feature = "metal"), not(feature = "vulkan")))]
use crate::core::{Errc, Error};
#[cfg(any(not(feature = "metal"), not(feature = "vulkan")))]
use crate::native::present::IGraphicsContext;
#[cfg(any(not(feature = "metal"), not(feature = "vulkan")))]
use std::ffi::c_void;

#[cfg(feature = "metal")]
use crate::native::presentation::graphics::wgpu_backend::create_metal;
#[cfg(all(feature = "metal", feature = "vulkan"))]
use crate::native::presentation::graphics::wgpu_backend::create_vulkan;

#[cfg(not(feature = "metal"))]
fn create_metal(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "graphics feature `metal` is disabled in this build",
    ))
}

#[cfg(not(all(feature = "metal", feature = "vulkan")))]
fn create_vulkan(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "macOS Vulkan requires the `metal` and `vulkan` features",
    ))
}

const METAL_STATUS: BackendStatus = if cfg!(feature = "metal") {
    BackendStatus::Active
} else {
    BackendStatus::Disabled
};

const VULKAN_STATUS: BackendStatus = if cfg!(all(feature = "metal", feature = "vulkan")) {
    BackendStatus::Active
} else {
    BackendStatus::Disabled
};

pub(crate) const PLATFORM_ENTRIES: &[GraphicsBackendEntry] = &[
    GraphicsBackendEntry {
        id: GraphicsBackend::Vulkan,
        priority: 30,
        status: VULKAN_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_vulkan,
    },
    GraphicsBackendEntry {
        id: GraphicsBackend::Metal,
        priority: 20,
        status: METAL_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_metal,
    },
];
