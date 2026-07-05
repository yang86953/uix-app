//! 渲染后端模块。

pub mod cpu;
pub mod gpu;
pub mod null;
pub mod traits;

pub use cpu::CpuBackend;
pub use gpu::GpuBackend;
pub use null::NullBackend;
pub use traits::{BackendCapabilities, BackendKind, DamageRegion, DrawSurface, RenderBackend};

use crate::native::{Error, IGraphicsContext};

/// 按种类创建后端实例。
pub fn create_backend(
    kind: BackendKind,
    gpu_ctx: Option<Box<dyn IGraphicsContext>>,
) -> Result<Box<dyn RenderBackend>, Error> {
    match kind {
        BackendKind::Cpu | BackendKind::Auto => Ok(Box::new(CpuBackend::new())),
        BackendKind::Null => Ok(Box::new(NullBackend::new())),
        BackendKind::Gpu => {
            let ctx = gpu_ctx.ok_or_else(|| {
                Error::new(
                    crate::native::Errc::InvalidArgument,
                    "Gpu 后端需要 IGraphicsContext",
                )
            })?;
            Ok(Box::new(GpuBackend::new(ctx)?))
        }
    }
}
