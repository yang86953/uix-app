//! 渲染后端模块。

pub mod contract;
pub mod cpu;
pub mod factory;
pub mod gpu;
#[cfg(test)]
pub(crate) mod test_backend;

pub use crate::core::DamageRegion;
pub use contract::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
pub use cpu::CpuBackend;
pub use gpu::GpuBackend;
#[cfg(test)]
pub(crate) use test_backend::TestBackend;

use crate::core::{Errc, Error};
use crate::native::traits::present::IGraphicsContext;

/// 按种类创建后端实例。
pub(crate) fn create_backend(
    kind: BackendKind,
    gpu_ctx: Option<Box<dyn IGraphicsContext>>,
) -> Result<Box<dyn RenderBackend>, Error> {
    match kind {
        BackendKind::Cpu | BackendKind::Auto => Ok(Box::new(CpuBackend::new())),
        #[cfg(test)]
        BackendKind::Test => Ok(Box::new(TestBackend::new())),
        BackendKind::Gpu => {
            let ctx = gpu_ctx.ok_or_else(|| {
                Error::new(Errc::InvalidArgument, "Gpu 后端需要 IGraphicsContext")
            })?;
            factory::create_native_raster_backend(ctx)
        }
    }
}
