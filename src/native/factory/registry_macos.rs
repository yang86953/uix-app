//! macOS backend registry table.

use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::graphics::metal;
use crate::native::traits::present::{GraphicsBackend, PresentMode, RasterMode};

const METAL_STATUS: BackendStatus = if cfg!(feature = "metal") {
    BackendStatus::Active
} else {
    BackendStatus::Disabled
};

pub(crate) const PLATFORM_ENTRIES: &[GraphicsBackendEntry] = &[GraphicsBackendEntry {
    id: GraphicsBackend::Metal,
    priority: 20,
    status: METAL_STATUS,
    raster: RasterMode::Cpu,
    present: PresentMode::PixelUpload,
    create: metal::create,
}];
