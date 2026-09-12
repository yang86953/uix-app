//! 渲染后端模块。

// 通用 GPU Renderer 的有限帧计划，供 backend 内部迁移使用。
#[cfg(feature = "graphics-gpu")]
pub(crate) mod frame_plan;
// 显式 GPU parity 通过真实 PaintContext/Canvas2D 入口验收共享 FramePlan。
#[cfg(any(uix_gpu_parity_vulkan, uix_gpu_parity_opengl, uix_gpu_parity_d3d11))]
#[path = "../../../tests-src/draw/backend/production_chain_parity.rs"]
pub(crate) mod production_chain_parity;
// CPU/GPU 离屏 Picture 池共用的槽位簿记。
pub(crate) mod slot_pool;
// 通用 RHI lowering，逐步替代 GPU backend 的逐 UI native submit。
#[cfg(feature = "graphics-gpu")]
pub(crate) mod rhi_renderer;

pub mod contract;
pub mod cpu;
#[cfg(feature = "graphics-gpu")]
pub mod gpu;

pub use crate::core::DamageRegion;
pub use contract::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
// test-harness 与 Agent 截屏只导出 API 无关的规范像素快照。
#[cfg(any(feature = "test-harness", feature = "agent-control"))]
pub use contract::SurfaceReadback;
pub use cpu::CpuBackend;
#[cfg(feature = "graphics-gpu")]
pub use gpu::GpuBackend;

use crate::core::{Errc, Error};

/// 按种类创建后端实例。
pub(crate) fn create_backend(kind: BackendKind) -> Result<Box<dyn RenderBackend>, Error> {
    // 通用工厂只负责不依赖原生 recipe 的后端。
    match kind {
        // Auto 已在上层解析，保留 CPU 映射作为防御性契约。
        BackendKind::Cpu | BackendKind::Auto => Ok(Box::new(CpuBackend::new())),
        // GPU 必须先经过原生 recipe factory 的能力与专用 owner 校验。
        BackendKind::Gpu => Err(Error::new(
            // 调用方缺少完整 recipe，属于可修正的参数错误。
            Errc::InvalidArgument,
            // 诊断明确指出唯一合法装配入口。
            "GPU 后端必须由已验证的原生图形配方构造",
        )),
    }
}
