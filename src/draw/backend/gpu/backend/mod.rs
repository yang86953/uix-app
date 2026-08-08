//! Native GPU surface 与后端资源、提交生命周期。
//!
//! 子模块划分（P2 行数治理）：[`surface`] `DrawSurface` 门面、[`impl_main`]
//! 生命周期与帧编码执行、[`impl_frame`] 帧命令绘制降级、[`render_backend`]
//! `RenderBackend` 实现、[`helpers`] 帧编码几何辅助、[`drop`] 析构。

pub(crate) mod drop;
pub(crate) mod helpers;
pub(crate) mod impl_frame;
pub(crate) mod impl_main;
pub(crate) mod render_backend;
// 叠加层 backdrop 快照独立管理，保持 RenderBackend 文件处于行数上限内。
pub(crate) mod render_backend_backdrop;
// 叠加层 backdrop 的薄 RHI 资源与命令契约由独立测试覆盖。
#[cfg(test)]
// 测试模块只编译 recording context，不进入生产依赖图。
mod rhi_backdrop_tests;
// 主 surface 的最终 present 状态机独立管理，保持 RenderBackend 文件可维护。
pub(crate) mod render_present;
pub(crate) mod rhi_frame;
pub(crate) mod rhi_submit;
// 主 surface 的持久 RHI 颜色目标独立管理代际和销毁。
pub(crate) mod rhi_surface;
// 主 surface 的局部 RHI 清理计划独立管理，避免污染提交主文件。
pub(crate) mod rhi_surface_clear;
// 主 surface 的滚动 TextureMove 计划独立管理，保持边界语义可审计。
pub(crate) mod rhi_surface_scroll;
// 主 surface 的 solid/mixed RHI 最终提交独立管理，控制提交文件行数。
pub(crate) mod rhi_surface_present;
// 主 surface 的 CPU soft segment RHI 合成独立管理，保持 retained 组合边界清晰。
pub(crate) mod rhi_surface_soft;
// Picture texture 的 RHI sampled blit 独立管理，保持目标切换边界清晰。
pub(crate) mod rhi_surface_blit;
pub(crate) mod surface;

pub use surface::NativeGpuDrawSurface;

use std::time::Instant;

use crate::core::{Error, PresentDamageTracker};
use crate::draw::backend::rhi_renderer::RhiRenderer;
use crate::native::present::{IGraphicsContext, OffscreenTargetId};
// 引入迁移期 RHI 的离屏纹理句柄。
use crate::native::present::rhi::TextureHandle;

use super::canvas::NativeGpuCanvas2D;

pub struct GpuBackend {
    pub(crate) gpu_ctx: Box<dyn IGraphicsContext>,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) surface: NativeGpuDrawSurface,
    // 离屏槽位只供 GPU backend 自身及其子模块访问，和元素类型保持同级可见性。
    pub(super) offscreens: Vec<Option<NativeGpuOffscreen>>,
    pub(crate) free_offscreen_ids: Vec<u32>,
    next_offscreen_id: u32,
    /// Picture paint currently targeting this offscreen handle id.
    pub(crate) active_offscreen: Option<u32>,
    /// RHI Picture target 是否已经执行过首个 Clear pass。
    offscreen_rhi_initialized: bool,
    /// The active Picture's latest checked flush completed. This gates eager
    /// CPU staging release at the checked target-restore boundary.
    offscreen_flush_committed: bool,
    /// A draw-time operation without a `Result` return path (for example an
    /// immediate Picture blit) failed.  The failure is reported from the sole
    /// final present boundary, so callers keep the frame dirty instead of
    /// treating an incomplete command stream as committed.
    frame_failure: Option<Error>,
    present_damage_tracker: PresentDamageTracker,
    soft_fallback_idle_deadline: Option<Instant>,
    soft_used_in_last_present: bool,
    gpu_only: bool,
    /// `new` is a crate-local hybrid fixture path and may receive an
    /// unprepared context; the production GPU registry always marks its
    /// factory-created context as prepared.
    factory_prepared: bool,
    pub(crate) shutdown: bool,
    /// D3D11 参考 adapter 的迁移期 FramePlan resource cache。
    pub(crate) rhi_renderer: Option<RhiRenderer>,
    /// 保存跨帧 retained surface 的 RHI 颜色纹理。
    pub(crate) rhi_surface_texture: Option<TextureHandle>,
    /// 记录 retained texture 所属的 surface generation 和 extent。
    pub(crate) rhi_surface_token: Option<crate::native::present::rhi::SurfaceToken>,
    /// 标记 FrameEncoder 已写入 retained texture、等待最终合成 present。
    pub(crate) rhi_surface_frame_pending_present: bool,
    /// 保存 overlay 干净背景的通用 RHI 纹理。
    pub(crate) rhi_overlay_backdrop_texture: Option<TextureHandle>,
    /// 记录 overlay 背景纹理所属的 surface generation 和 extent。
    pub(crate) rhi_overlay_backdrop_token: Option<crate::native::present::rhi::SurfaceToken>,
}

pub(super) struct NativeGpuOffscreen {
    // 保存兼容 presenter 使用的原生离屏 target。
    target: OffscreenTargetId,
    // 保存可选的 RHI texture；存在时由 FramePlan 负责绘制和采样。
    pub(crate) rhi_texture: Option<TextureHandle>,
    pub(crate) canvas: NativeGpuCanvas2D,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

/// Canvas coordinates are logical pixels. Native contexts report their
/// drawable extent through `width`/`height`, so derive the matching logical
/// extent from their single DPR source before allocating draw-side state.
pub(super) fn device_pixel_ratio_from_context(gpu_ctx: &dyn IGraphicsContext) -> f32 {
    let dpr = gpu_ctx.caps().device_pixel_ratio;
    if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    }
}

pub(super) fn logical_extent_from_context(gpu_ctx: &dyn IGraphicsContext) -> (i32, i32) {
    let dpr = device_pixel_ratio_from_context(gpu_ctx);
    let logical = |drawable: i32| ((drawable.max(1) as f32 / dpr).round() as i32).max(1);
    (logical(gpu_ctx.width()), logical(gpu_ctx.height()))
}
