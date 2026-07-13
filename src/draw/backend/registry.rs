//! Table-driven pairing of [`GraphicsBackend`] to [`RenderBackend`] (P6.7 M6 / P6.8).

use crate::core::{Errc, Error, Result};
use crate::draw::backend::native_gpu::NativeGpuBackend;
use crate::draw::backend::traits::RenderBackend;
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
            format!("RenderBackendRegistry: expected RasterMode::GpuNative, got {raster}"),
        ));
    }

    match ctx.caps().backend {
        GraphicsBackend::OpenGlEs | GraphicsBackend::D3d11 | GraphicsBackend::D3d12 => {
            NativeGpuBackend::new(ctx).map(|backend| Box::new(backend) as _)
        }
        GraphicsBackend::Metal => {
            ctx.try_shutdown()?;
            Err(Error::new(
                Errc::NotImplemented,
                "RenderBackendRegistry: Metal native raster is planned (use Cpu × PixelUpload context)",
            ))
        }
        other => {
            ctx.try_shutdown()?;
            Err(Error::new(
                Errc::InvalidArgument,
                format!("RenderBackendRegistry: no native raster backend for {other}"),
            ))
        }
    }
}

#[cfg(test)]
#[path = "../../tests/draw/backend/registry.rs"]
mod tests;
