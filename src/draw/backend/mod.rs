//! 渲染后端模块。

// 通用 GPU Renderer 的有限帧计划，供 backend 内部迁移使用。
pub(crate) mod frame_plan;
// 通用 RHI lowering，逐步替代 GPU backend 的逐 UI native submit。
pub(crate) mod rhi_renderer;

pub mod contract;
pub mod cpu;
pub mod factory;
pub mod gpu;

pub use crate::core::DamageRegion;
pub use contract::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
pub use cpu::CpuBackend;
pub use gpu::GpuBackend;

use crate::core::{Errc, Error};
use crate::native::present::IGraphicsContext;

/// 按种类创建后端实例。
pub(crate) fn create_backend(
    kind: BackendKind,
    gpu_ctx: Option<Box<dyn IGraphicsContext>>,
) -> Result<Box<dyn RenderBackend>, Error> {
    match kind {
        BackendKind::Cpu | BackendKind::Auto => Ok(Box::new(CpuBackend::new())),
        BackendKind::Gpu => {
            let ctx = gpu_ctx.ok_or_else(|| {
                Error::new(Errc::InvalidArgument, "Gpu 后端需要 IGraphicsContext")
            })?;
            factory::create_native_raster_backend(ctx)
        }
    }
}
