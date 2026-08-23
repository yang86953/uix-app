//! 帧渲染调度 — 从 UI event_loop 迁入的渲染段（Phase 3）。

use crate::core::{Errc, Error, Point, Rect};

use crate::core::DirtyRegion;

use crate::draw::backend::DamageRegion;
use crate::draw::debug::{DebugFrameSnapshot, DebugRenderService};
use crate::draw::painting::{EncodedFrameExecution, FrameImage, recorder::CommandRecorder};
use crate::draw::renderer::{InvalidationSource, RenderMetrics};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::image::ImageService;
use crate::draw::scene::{LayerTree, RenderObjectTree, ScenePaint};
use crate::draw::{FontHandle, RenderOutcome};
use crate::draw::{RasterPipeline, RenderTarget, ScrollCopy, UpdateStrategy};
// 帧诊断需要单调时钟与时长类型。
use std::time::{Duration, Instant};

/// 单帧渲染输入。
pub struct FrameRenderInput<'a> {
    /// 是否已经成功渲染过首帧。
    pub rendered_first: bool,
    /// 本帧需要重新绘制的脏区域。
    pub dirty_region: &'a DirtyRegion,
    /// 调用方观察到的场景树版本。
    pub tree_version: u64,
    /// 可选的滚动复制操作，每项包含区域及水平、垂直位移。
    pub scroll_move: Option<Vec<(Rect, f32, f32)>>,
    /// 本帧使用的默认字体句柄。
    pub font: FontHandle,
    /// 提供字体与文本资源的服务。
    pub font_service: &'a FontService,
    /// 提供图像资源的服务。
    pub image_service: &'a ImageService,
    /// 是否启用调试绘制。
    pub debug_mode: bool,
    /// 可选的当前悬停位置。
    pub hover_pos: Option<Point>,
    /// 可选的渲染指标收集器。
    pub metrics: Option<&'a RenderMetrics>,
    /// 上一已完成帧的无文本诊断快照。
    pub debug_frame: Option<&'a DebugFrameSnapshot>,
    /// 调用方根据事件、动画与布局事实给出的本帧失效来源。
    pub invalidation_source: InvalidationSource,
}

/// 单帧渲染输出。
pub struct FrameRenderOutput {
    /// 本帧的渲染、空闲或失败结果。
    pub outcome: RenderOutcome,
    /// 触发本帧工作的失效来源。
    pub inv_source: InvalidationSource,
    /// 本帧处理的场景树版本。
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
    // 帧诊断：最近一帧 GPU 路径的记录阶段耗时（场景遍历 + 绘制编码）。
    last_record_us: Duration,
    // 帧诊断：最近一帧 GPU 路径的提交阶段耗时（end_frame 提交与 present 等待）。
    last_submit_us: Duration,
}

// 帧诊断：返回最近一帧 GPU 路径的记录/提交阶段耗时。
impl ScenePipeline {
    /// 返回最近一帧 GPU 路径的记录/提交阶段耗时，供帧诊断定位卡顿段。
    pub(crate) fn last_frame_stage_times(&self) -> (Duration, Duration) {
        (self.last_record_us, self.last_submit_us)
    }
}

mod damage;
// GPU overlay backdrop 的中间提交与统一失败边界。
mod backdrop_refresh;
mod pipeline;
// 统一拥有 recorder 参考尺寸与初始化生命周期。
mod surface;
// 单元测试锁定 backdrop typed failure 的场景边界传播。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/draw/renderer/scene_pipeline/tests.rs"]
mod tests;

use self::damage::*;
