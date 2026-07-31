//! Linux backend registry table.

use crate::diagnostics::PendingFailureQueue;
use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::present::{GraphicsBackend, PresentMode, RasterMode};

#[cfg(any(not(feature = "opengles"), not(feature = "vulkan")))]
use crate::core::{Errc, Error};
#[cfg(any(not(feature = "opengles"), not(feature = "vulkan")))]
use crate::native::present::IGraphicsContext;
#[cfg(any(not(feature = "opengles"), not(feature = "vulkan")))]
use std::ffi::c_void;

#[cfg(feature = "opengles")]
use crate::native::presentation::graphics::wgpu_backend::create_opengl as create_opengles;
#[cfg(feature = "vulkan")]
use crate::native::presentation::graphics::wgpu_backend::create_vulkan;

#[cfg(not(feature = "opengles"))]
fn create_opengles(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(feature_disabled("opengles"))
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

#[cfg(any(not(feature = "opengles"), not(feature = "vulkan")))]
fn feature_disabled(feature: &str) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("graphics feature `{feature}` is disabled in this build"),
    )
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
