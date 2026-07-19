//! Windows backend registry table.

use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::traits::present::{GraphicsBackend, PresentMode, RasterMode};

#[cfg(any(
    not(feature = "d3d12"),
    not(feature = "opengles"),
    not(feature = "vulkan")
))]
use crate::core::{Errc, Error};
#[cfg(any(
    not(feature = "d3d12"),
    not(feature = "opengles"),
    not(feature = "vulkan")
))]
use crate::native::traits::present::IGraphicsContext;
#[cfg(any(
    not(feature = "d3d12"),
    not(feature = "opengles"),
    not(feature = "vulkan")
))]
use std::ffi::c_void;

#[cfg(feature = "d3d12")]
use crate::native::graphics::wgpu_backend::create_d3d12;
#[cfg(feature = "opengles")]
use crate::native::graphics::wgpu_backend::create_opengl as create_opengles;
#[cfg(feature = "vulkan")]
use crate::native::graphics::wgpu_backend::create_vulkan;

#[cfg(not(feature = "d3d12"))]
fn create_d3d12(_: *mut c_void, _: i32, _: i32) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(feature_disabled("d3d12"))
}

#[cfg(not(feature = "opengles"))]
fn create_opengles(_: *mut c_void, _: i32, _: i32) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(feature_disabled("opengles"))
}

#[cfg(not(feature = "vulkan"))]
fn create_vulkan(_: *mut c_void, _: i32, _: i32) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(feature_disabled("vulkan"))
}

#[cfg(any(
    not(feature = "d3d12"),
    not(feature = "opengles"),
    not(feature = "vulkan")
))]
fn feature_disabled(feature: &str) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("graphics feature `{feature}` is disabled in this build"),
    )
}

const D3D12_STATUS: BackendStatus = if cfg!(feature = "d3d12") {
    BackendStatus::Active
} else {
    BackendStatus::Disabled
};

const OPENGL_STATUS: BackendStatus = if cfg!(feature = "opengles") {
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
        id: GraphicsBackend::Vulkan,
        priority: 30,
        status: VULKAN_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_vulkan,
    },
    GraphicsBackendEntry {
        id: GraphicsBackend::D3d12,
        priority: 20,
        status: D3D12_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_d3d12,
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
