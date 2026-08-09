//! Table-driven pairing of a native graphics API to [`RenderBackend`] (P6.7 M6 / P6.8).

use crate::core::{Error, Result};
use crate::draw::backend::contract::RenderBackend;
use crate::draw::backend::gpu::GpuBackend;
use crate::native::present::{GpuRecipeOwner, IGraphicsContext};

/// Creates the GPU raster backend for a native graphics context.
///
/// Only used when `caps().raster == GpuNative`; upload-present paths
/// keep a fixed CPU raster backend.
pub(crate) fn create_native_raster_backend(
    ctx: Box<dyn IGraphicsContext>,
) -> Result<Box<dyn RenderBackend>, Error> {
    // 在进入 draw backend 前把可选兼容视图收敛为已验证 GPU recipe owner。
    let owner = GpuRecipeOwner::try_new(ctx)?;
    // 所有具体 GraphicsApi 共用唯一 GPU backend，差异只留在薄 RHI owner 内。
    GpuBackend::new_gpu_only(owner).map(|backend| Box::new(backend) as _)
}
