//! Table-driven pairing of [`GraphicsBackend`] to [`RenderBackend`] (P6.7 M6 / P6.8).

use crate::core::{Errc, Error, Result};
use crate::draw::backend::contract::RenderBackend;
use crate::draw::backend::gpu::GpuBackend;
use crate::native::traits::present::{GraphicsBackend, IGraphicsContext, RasterMode};

/// Creates the GPU raster backend for a native graphics context.
///
/// Only used when `caps().raster == GpuNative`; upload-present paths
/// keep a fixed CPU raster backend.
pub(crate) fn create_native_raster_backend(
    mut ctx: Box<dyn IGraphicsContext>,
) -> Result<Box<dyn RenderBackend>, Error> {
    if ctx.caps().raster != RasterMode::GpuNative {
        let raster = ctx.caps().raster;
        ctx.try_shutdown()?;
        return Err(Error::new(
            Errc::InvalidArgument,
            format!("RenderBackendFactory: expected RasterMode::GpuNative, got {raster}"),
        ));
    }

    match ctx.caps().backend {
        GraphicsBackend::OpenGlEs
        | GraphicsBackend::D3d11
        | GraphicsBackend::D3d12
        | GraphicsBackend::Vulkan
        | GraphicsBackend::Metal => {
            GpuBackend::new_gpu_only(ctx).map(|backend| Box::new(backend) as _)
        }
        other => {
            ctx.try_shutdown()?;
            Err(Error::new(
                Errc::InvalidArgument,
                format!("RenderBackendFactory: no native raster backend for {other}"),
            ))
        }
    }
}
