//! macOS backend registry table.

use crate::native::factory::registry::{BackendStatus, GraphicsBackendEntry};
use crate::native::traits::present::{GraphicsBackend, PresentMode, RasterMode};

#[cfg(not(feature = "metal"))]
use crate::core::{Errc, Error};
#[cfg(not(feature = "metal"))]
use crate::native::traits::present::IGraphicsContext;
#[cfg(not(feature = "metal"))]
use std::ffi::c_void;

#[cfg(feature = "metal")]
use crate::native::graphics::metal::create as create_metal;

#[cfg(not(feature = "metal"))]
fn create_metal(_: *mut c_void, _: i32, _: i32) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "graphics feature `metal` is disabled in this build",
    ))
}

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
    create: create_metal,
}];
