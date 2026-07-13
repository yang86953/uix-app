//! Maps a native [`IGraphicsContext`] to the draw [`GraphicsEngine`] via orthogonal axes.

use crate::core::{Errc, Error, Result};
use crate::draw::engine::present_upload::PresentUploadEngine;
use crate::draw::gpu_engine::GpuEngine;
use crate::draw::traits::GraphicsEngine;
use crate::native::traits::present::{IGraphicsContext, PresentMode, RasterMode};

/// Creates the draw engine that matches `context.caps().raster` × `present`.
///
/// Legal combinations (table-driven):
/// - `GpuNative` × `Swapchain` → [`GpuEngine`]
/// - `Cpu` × `PixelUpload` → [`PresentUploadEngine`]
///
/// `PresentMode::CpuPresenter` has no [`IGraphicsContext`] and is rejected here
/// (app builds [`crate::draw::SoftwareEngine`] + [`crate::native::traits::IPresenter`]).
///
/// On failure the context is shut down before the error is returned (M3).
pub(crate) fn create_graphics_engine(
    mut context: Box<dyn IGraphicsContext>,
) -> Result<Box<dyn GraphicsEngine>, Error> {
    let caps = context.caps();
    match (caps.raster, caps.present) {
        (RasterMode::Cpu, PresentMode::PixelUpload) => PresentUploadEngine::new(context)
            .map(|engine| Box::new(engine) as Box<dyn GraphicsEngine>),
        (RasterMode::GpuNative, PresentMode::Swapchain) => {
            GpuEngine::new(context).map(|engine| Box::new(engine) as Box<dyn GraphicsEngine>)
        }
        (raster, PresentMode::CpuPresenter) => {
            context.try_shutdown()?;
            Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "create_graphics_engine: PresentMode::CpuPresenter has no IGraphicsContext \
                     (raster={raster})"
                ),
            ))
        }
        (raster, present) => {
            context.try_shutdown()?;
            Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "create_graphics_engine: unsupported RasterMode × PresentMode \
                     combination: {raster} × {present}"
                ),
            ))
        }
    }
}

