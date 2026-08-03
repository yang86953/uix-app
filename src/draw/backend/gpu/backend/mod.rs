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
pub(crate) mod surface;

pub use surface::NativeGpuDrawSurface;

use std::sync::Arc;
use std::time::Instant;

use crate::core::{Error, PresentDamageTracker, Rect};
use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::painting::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FrameEncoder, FrameEncoderError,
    FrameGlyphBlit, FrameImage, FrameRasterOp, FrameRect, FrameStrokeRect,
};
use crate::draw::Canvas2D;
use crate::draw::backend::contract::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
use crate::native::present::{IGraphicsContext, NativeRasterCaps, OffscreenTargetId};

use super::canvas::NativeGpuCanvas2D;

pub struct GpuBackend {
    pub(crate) gpu_ctx: Box<dyn IGraphicsContext>,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) surface: NativeGpuDrawSurface,
    pub(crate) offscreens: Vec<Option<NativeGpuOffscreen>>,
    pub(crate) free_offscreen_ids: Vec<u32>,
    next_offscreen_id: u32,
    /// Picture paint currently targeting this offscreen handle id.
    pub(crate) active_offscreen: Option<u32>,
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
}

pub(super) struct NativeGpuOffscreen {
    target: OffscreenTargetId,
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
