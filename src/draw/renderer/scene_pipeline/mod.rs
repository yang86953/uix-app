//! 帧渲染调度 — 从 UI event_loop 迁入的渲染段（Phase 3）。

use crate::core::{Errc, Error, Point, Rect};

use crate::core::DirtyRegion;

use crate::draw::backend::DamageRegion;
use crate::draw::debug::DebugRenderService;
use crate::draw::painting::{EncodedFrameExecution, FrameImage, recorder::CommandRecorder};
use crate::draw::renderer::{InvalidationSource, RenderMetrics};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::font::text::TextRenderService;
use crate::draw::resources::image::ImageService;
use crate::draw::scene::{LayerTree, RenderObjectTree, ScenePaint};
use crate::draw::{Color, FontHandle, RenderOutcome};
use crate::draw::{RasterPipeline, RenderTarget, ScrollCopy, UpdateStrategy};

/// 单帧渲染输入。
pub struct FrameRenderInput<'a> {
    pub rendered_first: bool,
    pub dirty_region: &'a DirtyRegion,
    pub tree_version: u64,
    pub scroll_move: Option<Vec<(Rect, f32, f32)>>,
    pub font: FontHandle,
    pub font_service: &'a FontService,
    pub image_service: &'a ImageService,
    pub debug_mode: bool,
    pub hover_pos: Option<Point>,
    pub metrics: Option<&'a RenderMetrics>,
}

/// 单帧渲染输出。
pub struct FrameRenderOutput {
    pub outcome: RenderOutcome,
    pub inv_source: InvalidationSource,
    pub tree_version: u64,
}

/// 帧渲染器 — 持有 LayerTree 与合成状态。
pub struct ScenePipeline {
    layer_tree: LayerTree,
    render_object_tree: RenderObjectTree,
    last_tree_version: u64,
    /// Private API-neutral producer for every scene path before the real
    /// backend consumes the one ordered main `FrameEncoder` (#181). It never
    /// owns a presentation surface and therefore cannot become a second
    /// submission boundary.
    recorder: CommandRecorder,
    recording_extent: Option<(i32, i32)>,
    /// Clean retained main surface captured immediately before the first
    /// root-level overlay frame clears it. Cloning FrameImage is cheap because
    /// its immutable pixels are shared.
    overlay_backdrop: Option<FrameImage>,
    /// Once the normal tree changes while an overlay is present, the current
    /// real surface already contains overlay pixels and can no longer become a
    /// clean backdrop. Wait for every overlay to leave before capturing again.
    overlay_backdrop_blocked: bool,
    /// 上一帧已经应用或明确降级的 typed overlay effect 计划。
    overlay_backdrop_effect: Option<crate::draw::OverlayBackdropEffect>,
    /// Prevents Picture handles created by one raster owner from being reused
    /// after bounded recovery switches the live engine.
    raster_pipeline: Option<RasterPipeline>,
}

mod damage;
mod pipeline;
// 单元测试锁定 backdrop typed failure 的场景边界传播。
#[cfg(test)]
mod tests;

use self::damage::*;
