//! Windows backend registry table.

use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::graphics::{d3d11, d3d12, opengl};
use crate::native::traits::present::{GraphicsBackend, PresentMode, RasterMode};

const D3D12_STATUS: BackendStatus = if cfg!(feature = "d3d12") {
    BackendStatus::Planned
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
        priority: 30,
        status: D3D12_STATUS,
        // Planned: GpuNative × Swapchain when implemented.
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: d3d12::create,
    },
    GraphicsBackendEntry {
        id: GraphicsBackend::D3d11,
        priority: 20,
        status: D3D11_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: d3d11::create,
    },
    GraphicsBackendEntry {
        id: GraphicsBackend::OpenGlEs,
        priority: 10,
        status: OPENGL_STATUS,
        raster: RasterMode::GpuNative,
        present: PresentMode::Swapchain,
        create: opengl::create,
    },
];
