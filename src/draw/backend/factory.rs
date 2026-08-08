//! Table-driven pairing of a native graphics API to [`RenderBackend`] (P6.7 M6 / P6.8).

use crate::core::{Errc, Error, Result};
use crate::draw::backend::contract::RenderBackend;
use crate::draw::backend::gpu::GpuBackend;
use crate::native::present::{IGraphicsContext, RasterMode};

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

    // 所有具体 GraphicsApi 共用唯一 GPU backend，差异只留在薄 RHI context。
    GpuBackend::new_gpu_only(ctx).map(|backend| Box::new(backend) as _)
}
