//! Windows backend registry table.

use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::traits::present::{GraphicsBackend, PresentMode, RasterMode};

#[cfg(any(
    not(feature = "d3d11"),
    not(feature = "d3d12"),
    not(feature = "opengles")
))]
use crate::core::{Errc, Error};
#[cfg(any(
    not(feature = "d3d11"),
    not(feature = "d3d12"),
    not(feature = "opengles")
))]
use crate::native::traits::present::IGraphicsContext;
#[cfg(any(
    not(feature = "d3d11"),
    not(feature = "d3d12"),
    not(feature = "opengles")
))]
use std::ffi::c_void;

#[cfg(feature = "d3d11")]
use crate::native::graphics::d3d11::create as create_d3d11;
#[cfg(feature = "d3d12")]
use crate::native::graphics::d3d12::create as create_d3d12;
#[cfg(feature = "opengles")]
use crate::native::graphics::opengl::create as create_opengles;

#[cfg(not(feature = "d3d11"))]
fn create_d3d11(_: *mut c_void, _: i32, _: i32) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(feature_disabled("d3d11"))
}

#[cfg(not(feature = "d3d12"))]
fn create_d3d12(_: *mut c_void, _: i32, _: i32) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(feature_disabled("d3d12"))
}

#[cfg(not(feature = "opengles"))]
fn create_opengles(_: *mut c_void, _: i32, _: i32) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(feature_disabled("opengles"))
}

#[cfg(any(
    not(feature = "d3d11"),
    not(feature = "d3d12"),
    not(feature = "opengles")
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

const D3D11_STATUS: BackendStatus = if cfg!(feature = "d3d11") {
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
        id: GraphicsBackend::D3d12,
        // First native slice stays behind the established D3D11/WGL paths.
        priority: 5,
        status: D3D12_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_d3d12,
    },
    GraphicsBackendEntry {
        id: GraphicsBackend::D3d11,
        priority: 20,
        status: D3D11_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: create_d3d11,
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
