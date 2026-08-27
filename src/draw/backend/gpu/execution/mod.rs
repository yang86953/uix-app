//! Native GPU surface 与后端资源、提交生命周期。
//!
//! 子模块划分（P2 行数治理）：[`surface`] `DrawSurface` 门面、[`impl_main`]
//! 生命周期与资源管理、[`render_backend`] `RenderBackend` 实现及 [`drop`] 析构。

pub(crate) mod drop;
pub(crate) mod impl_main;
pub(crate) mod render_backend;
// 叠加层 backdrop 快照独立管理，保持 RenderBackend 文件处于行数上限内。
pub(crate) mod render_backend_backdrop;
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
// 最终 retained-to-swapchain 合成集中校验 damage 并生成逐矩形 scissor。
pub(crate) mod rhi_surface_composite;
// retained texture 到真实 surface 的最终合成与测试观察时序独立管理。
pub(crate) mod rhi_surface_final;
// 主 surface 的 CPU soft segment RHI 合成独立管理，保持 retained 组合边界清晰。
pub(crate) mod rhi_surface_soft;
// Picture texture 的 RHI sampled blit 独立管理，保持目标切换边界清晰。
pub(crate) mod rhi_surface_blit;
pub(crate) mod surface;

pub use surface::NativeGpuDrawSurface;

use std::time::Instant;

use crate::core::{Error, PresentDamageTracker, PresentSurface};
use crate::draw::backend::rhi_renderer::RhiRenderer;
// test-harness 在 backend 内暂存最终合成后的规范像素结果。
#[cfg(feature = "test-harness")]
use crate::draw::backend::SurfaceReadback;
use crate::platform::presentation::rhi::GpuRecipeOwner;
// 引入迁移期 RHI 的离屏纹理句柄。
use crate::platform::presentation::rhi::TextureHandle;

use super::canvas::NativeGpuCanvas2D;

/// 拥有原生 GPU 绘制表面、RHI 渲染器及离屏资源的后端。
pub struct GpuBackend {
    // 保存构造期已验证的 GPU recipe owner，不直接依赖迁移期兼容 trait object。
    pub(crate) gpu_ctx: GpuRecipeOwner,
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
    pub(crate) shutdown: bool,
    /// 参考 adapter 共用的 FramePlan resource cache。
    pub(crate) rhi_renderer: Option<RhiRenderer>,
    /// 保存跨帧 retained surface 的 RHI 颜色纹理。
    pub(crate) rhi_surface_texture: Option<TextureHandle>,
    /// 记录 retained texture 所属的 surface generation 和 extent。
    pub(crate) rhi_surface_token: Option<crate::platform::presentation::rhi::SurfaceToken>,
    /// 标记 FrameEncoder 已写入 retained texture、等待最终合成 present。
    pub(crate) rhi_surface_frame_pending_present: bool,
    // 标记下一次最终合成需要在 present 前读取真实 surface。
    #[cfg(feature = "test-harness")]
    pub(crate) surface_readback_requested: bool,
    // 保存同一最终呈现事务产生的规范像素或 typed failure。
    #[cfg(feature = "test-harness")]
    pub(crate) surface_readback_result: Option<Result<SurfaceReadback, Error>>,
    // 记录当前帧由共享 FramePlan 成功执行的主表面 TextureMove 数量。
    #[cfg(feature = "test-harness")]
    pub(crate) executed_texture_moves_in_frame: usize,
    /// 保存 overlay 干净背景的通用 RHI 纹理。
    pub(crate) rhi_overlay_backdrop_texture: Option<TextureHandle>,
    /// 保存从干净背景复制并应用当前 blur 策略的 RHI 纹理。
    pub(crate) rhi_overlay_backdrop_effect_texture: Option<TextureHandle>,
    /// 记录 overlay 背景纹理所属的 surface generation 和 extent。
    pub(crate) rhi_overlay_backdrop_token: Option<crate::platform::presentation::rhi::SurfaceToken>,
}

pub(super) struct NativeGpuOffscreen {
    // 保存唯一的 RHI 离屏纹理所有权，绘制和采样都经由 FramePlan。
    pub(crate) rhi_texture: TextureHandle,
    pub(crate) canvas: NativeGpuCanvas2D,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

// 从单一 surface 快照规范化 draw backend 使用的设备像素比。
pub(super) fn device_pixel_ratio_from_surface(surface: PresentSurface) -> f32 {
    // 只接受可稳定映射逻辑坐标的有限正 DPR。
    let dpr = surface.device_pixel_ratio;
    // 有效 DPR 原样进入 canvas 空间换算。
    if dpr.is_finite() && dpr > 0.0 {
        // 返回 native context 报告的真实比例。
        dpr
    } else {
        // 无效 native 元数据保守退回 identity 比例。
        1.0
    }
}

// 从同一个 PresentSurface 快照派生逻辑 extent 与 DPR。
pub(super) fn logical_metadata_from_surface(surface: PresentSurface) -> ((i32, i32), f32) {
    // 先规范化快照内的设备像素比。
    let dpr = device_pixel_ratio_from_surface(surface);
    // 使用同一比例把物理 drawable 尺寸转换为逻辑尺寸。
    let logical = |drawable: i32| ((drawable.max(1) as f32 / dpr).round() as i32).max(1);
    // 返回不可撕裂的逻辑 extent 与对应 DPR。
    (
        // 同时换算快照内的物理宽高。
        (
            logical(surface.drawable_width),
            logical(surface.drawable_height),
        ),
        // 保留本次快照使用的规范化 DPR。
        dpr,
    )
}
