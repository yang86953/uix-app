//! Linux backend registry table.

use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::graphics::{opengl, vulkan};
use crate::native::traits::present::{GraphicsBackend, PresentMode, RasterMode};

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
        raster: RasterMode::Cpu,
        present: PresentMode::PixelUpload,
        create: vulkan::create,
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
