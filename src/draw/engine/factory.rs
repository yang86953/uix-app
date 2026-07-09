//! Maps a native [`IGraphicsContext`] to the draw [`GraphicsEngine`] for its pipeline profile.

use crate::core::{Errc, Error, Result};
use crate::draw::engine::present_upload::PresentUploadEngine;
use crate::draw::gpu_engine::GpuEngine;
use crate::draw::traits::GraphicsEngine;
use crate::native::traits::present::{IGraphicsContext, RenderPipelineProfile};

/// Creates the draw engine that matches `context.caps().pipeline`.
///
/// On failure the context is shut down before the error is returned (M3).
pub fn create_graphics_engine(
    mut context: Box<dyn IGraphicsContext>,
) -> Result<Box<dyn GraphicsEngine>, Error> {
    match context.caps().pipeline {
        RenderPipelineProfile::CpuUploadPresent => PresentUploadEngine::new(context)
            .map(|engine| Box::new(engine) as Box<dyn GraphicsEngine>),
        RenderPipelineProfile::NativeGpuRaster => {
            GpuEngine::new(context).map(|engine| Box::new(engine) as Box<dyn GraphicsEngine>)
        }
        RenderPipelineProfile::CpuPresenter => {
            context.shutdown();
            Err(Error::new(
                Errc::InvalidArgument,
                "create_graphics_engine: CpuPresenter has no IGraphicsContext",
            ))
        }
    }
}
