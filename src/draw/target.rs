//! 渲染目标协议 — 帧生命周期、更新策略与后端能力。

pub use crate::draw::outcome::RenderOutcome;

use crate::core::{Error, Point, Rect};
use crate::draw::Canvas2D;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::painting::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::native::present::PresentTestResult;
use std::time::Instant;

/// 一次保留缓冲内的滚动像素移动。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollCopy {
    pub viewport: Rect,
    pub delta: Point,
}

impl ScrollCopy {
    pub const fn new(viewport: Rect, dx: f32, dy: f32) -> Self {
        Self {
            viewport,
            delta: Point::new(dx, dy),
        }
    }
}

/// 帧更新策略。
#[derive(Debug, Clone)]
pub enum UpdateStrategy {
    FullRedraw,
    DirtyRects(Vec<Rect>),
    /// 先移动滚动像素，再清除并重绘暴露区。
    ///
    /// 仅允许具备 `scroll_memmove` 的局部重绘后端执行；其余后端必须
    /// 在帧边界提升为 `FullRedraw`，不能静默跳过像素移动。
    ScrollCopies {
        dirty_rects: Vec<Rect>,
        copies: Vec<ScrollCopy>,
    },
}

impl UpdateStrategy {
    pub fn rects(&self) -> Option<&[Rect]> {
        match self {
            UpdateStrategy::FullRedraw => None,
            UpdateStrategy::DirtyRects(rects) => Some(rects),
            UpdateStrategy::ScrollCopies { dirty_rects, .. } => Some(dirty_rects),
        }
    }

    pub fn should_clear(&self) -> bool {
        true
    }
}

/// 帧最终呈现方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationMode {
    /// 引擎输出 CPU 像素缓冲，由 platform presenter 提交到窗口。
    ExternalPresenter,
    /// 引擎内部已完成呈现，例如 GPU 后端自行 swap buffers。
    BackendManaged,
}

/// 图形引擎能力声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsCapabilities {
    pub presentation_mode: PresentationMode,
    pub partial_redraw: bool,
    /// 是否支持 Picture 离屏缓存（`create_offscreen` / blit）。
    pub offscreen: bool,
    /// 保留缓冲是否支持帧内重叠安全的滚动像素移动。
    pub scroll_memmove: bool,
}

impl GraphicsCapabilities {
    pub fn cpu_pixels() -> Self {
        Self {
            presentation_mode: PresentationMode::ExternalPresenter,
            partial_redraw: true,
            offscreen: true,
            scroll_memmove: true,
        }
    }

    pub fn backend_managed_full_redraw() -> Self {
        Self {
            presentation_mode: PresentationMode::BackendManaged,
            partial_redraw: false,
            offscreen: false,
            scroll_memmove: false,
        }
    }

    /// Backend-managed present + CPU 离屏（PresentUpload 等）。
    pub fn backend_managed_with_offscreen() -> Self {
        Self {
            presentation_mode: PresentationMode::BackendManaged,
            partial_redraw: false,
            offscreen: true,
            scroll_memmove: false,
        }
    }

    /// Backend-managed present + 跨帧保留的 CPU 像素与离屏表面。
    pub fn backend_managed_retained_pixels() -> Self {
        Self {
            presentation_mode: PresentationMode::BackendManaged,
            partial_redraw: true,
            offscreen: true,
            scroll_memmove: true,
        }
    }

    pub fn uses_external_presenter(self) -> bool {
        matches!(self.presentation_mode, PresentationMode::ExternalPresenter)
    }

    pub fn supports_partial_redraw(self) -> bool {
        self.partial_redraw
    }

    pub fn supports_offscreen(self) -> bool {
        self.offscreen
    }

    pub fn supports_scroll_memmove(self) -> bool {
        self.partial_redraw && self.scroll_memmove
    }
}

/// Raster provenance selected for the live renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterPipeline {
    Cpu,
    /// Every final UI pixel is produced by native GPU draw commands. CPU work
    /// is limited to scene construction, layout, decoding and tessellation.
    GpuNative,
}

/// 图形引擎 — 帧生命周期与离屏缓冲管理。
pub trait RenderTarget: 'static {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error>;
    /// Checked teardown boundary. Callers and Drop paths must use this method
    /// so native failures stay typed for recovery instead of becoming log-only.
    fn try_shutdown(&mut self) -> Result<(), Error>;
    /// Resize is a graphics lifecycle operation and must propagate a typed
    /// failure. Callers retain invalidation and enter bounded recovery.
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
    /// 开始一帧；`FrameReady` 携带引擎实际采用的清除/裁剪区域，可能比请求策略更保守。
    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome;
    /// `present_damage` 为合成层计算的呈现损伤；BackendManaged 后端用于 swap/present。
    fn end_frame(&mut self, present_damage: &crate::draw::backend::DamageRegion) -> RenderOutcome;

    /// Tests an already-occluded backend-managed swapchain without drawing or
    /// submitting frame data. Ordinary engines never need to override this.
    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        Err(Error::new(
            crate::core::Errc::NotImplemented,
            "render target does not support idle present tests",
        ))
    }

    /// test-harness 将一次可控 device-lost 注入安排到真实 backend 边界。
    #[cfg(feature = "test-harness")]
    fn inject_graphics_device_lost_for_test(&mut self) -> Result<(), Error> {
        Err(Error::new(
            crate::core::Errc::NotImplemented,
            "render target does not expose a lower graphics fault injection",
        ))
    }

    /// test-harness 将一次可控 surface-lost 注入安排到真实 surface 边界。
    #[cfg(feature = "test-harness")]
    fn inject_graphics_surface_lost_for_test(&mut self) -> Result<(), Error> {
        Err(Error::new(
            crate::core::Errc::NotImplemented,
            "render target does not expose a lower surface fault injection",
        ))
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D;

    /// 引擎当前采用的 logical viewport；默认等于 Canvas2D extent。
    ///
    /// drawable-sized CPU upload 引擎必须覆盖该方法，避免通过单一 DPR
    /// 反推两个轴时引入独立取整误差。
    fn logical_extent(&mut self) -> (i32, i32) {
        let canvas = self.canvas_2d();
        (canvas.width(), canvas.height())
    }

    /// The sole external platform presenter has accepted the pending frame.
    /// Backend-managed paths must never receive this callback.
    fn external_present_succeeded(&mut self) {}

    /// The external platform presenter rejected a pending frame. The default
    /// is intentionally inert; recovery wrappers record this typed failure at
    /// the next frame boundary while ordinary CPU renderers retain dirty.
    fn external_present_failed(&mut self, _error: Error) {}

    /// Records the successful final presentation time used to arm optional
    /// non-visual resource maintenance. Implementations must not request a
    /// frame merely to service this deadline.
    fn note_presented_at(&mut self, _now: Instant) {}

    /// Earliest one-shot deadline for releasing idle, recreatable resources.
    fn idle_resource_deadline(&self) -> Option<Instant> {
        None
    }

    /// Releases idle resources whose deadline is due. This is non-visual
    /// maintenance: it must not draw, submit, present, or change retained UI
    /// state.
    fn release_idle_resources(&mut self, _now: Instant) {}

    /// 恢复包装器是否已耗尽全部动作；窗口调度器据此进入无 deadline 终态。
    fn has_terminal_failure(&self) -> bool {
        false
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        GraphicsCapabilities::cpu_pixels()
    }

    fn raster_pipeline(&self) -> RasterPipeline {
        RasterPipeline::Cpu
    }

    fn dpi(&self) -> f32 {
        96.0
    }
    fn device_pixel_ratio(&self) -> f32 {
        1.0
    }
    fn orientation(&self) -> crate::draw::geometry::spatial::Orientation {
        crate::draw::geometry::spatial::Orientation::YDown
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        let _ = (width, height);
        None
    }
    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        let _ = handle;
    }

    /// Checked counterpart of [`Self::destroy_offscreen`]. Production
    /// compositor paths use this boundary to preserve an offscreen handle
    /// when native destruction fails.
    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        self.destroy_offscreen(handle);
        Ok(())
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let _ = handle;
        None
    }
    fn blit_offscreen_to_canvas(
        &mut self,
        _handle: &ImageHandle,
        _src_rect: Rect,
        _dst_rect: Rect,
        _canvas: &mut dyn Canvas2D,
    ) {
    }
    fn copy_offscreen_pixels(&self, _handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        None
    }

    /// Copies the current retained main-surface pixels before the next
    /// `begin_frame` may clear them. Engines without a readable retained CPU
    /// surface leave this unsupported so callers preserve the full-redraw
    /// path.
    fn copy_frame_pixels(&self) -> Option<(Vec<u32>, i32)> {
        None
    }

    /// GPU 保留色缓冲快照为 overlay 干净背景（无 CPU readback）。
    /// 资源与设备失败保持 typed error，交由 renderer recovery 处理。
    /// 成功后 [`Self::has_overlay_backdrop`] 为 true；不支持时返回 `Ok(false)`。
    fn snapshot_overlay_backdrop(&mut self) -> Result<bool, Error> {
        Ok(false)
    }

    /// 对引擎持有的 overlay 干净背景执行区域高斯模糊，不获取或呈现主表面。
    fn blur_overlay_backdrop(&mut self, _region: Rect, _radius: f32) -> Result<bool, Error> {
        // 普通 target 没有 GPU backdrop，向调用方报告未执行。
        Ok(false)
    }

    /// 将 overlay 背景写回主表面并取消本帧全幅 clear；提交失败保持 typed error。
    fn restore_overlay_backdrop(&mut self) -> Result<bool, Error> {
        Ok(false)
    }

    /// 检查式释放引擎持有的 overlay 背景快照。
    fn release_overlay_backdrop(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// 是否持有有效的 overlay 背景快照（CPU `FrameImage` 路径不经此查询）。
    fn has_overlay_backdrop(&self) -> bool {
        false
    }

    /// Executes a lossless API-neutral encoded Picture on a backend that
    /// explicitly supports it. `Unsupported` is a normal fallback result;
    /// compositor code then uses full DisplayList replay.
    fn try_execute_encoded_picture(
        &mut self,
        _handle: &ImageHandle,
        _encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        Ok(EncodedPictureExecution::Unsupported)
    }

    /// Executes the complete API-neutral main frame without presenting it.
    /// A backend that does not implement this contract must report
    /// `Unsupported`; ScenePipeline deliberately treats that as a typed frame
    /// failure rather than resuming a direct backend-canvas path.
    fn try_execute_encoded_frame(
        &mut self,
        _encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        Ok(EncodedFrameExecution::Unsupported)
    }

    /// Checked Picture begin boundary. Unsupported targets fail explicitly.
    // 默认 target 不允许把缺失的 Picture 生命周期伪装成成功。
    fn try_begin_offscreen_paint(
        // 接收当前 render target 的唯一可变 owner。
        &mut self,
        // 接收调用方已经创建的 Picture 目标身份。
        handle: &ImageHandle,
        // 返回 typed failure，供恢复包装器处理。
    ) -> Result<(), Error> {
        // 默认实现只消费参数，具体 target 必须显式覆盖生命周期。
        let _ = handle;
        // 缺失 Picture begin 属于明确的能力缺口。
        Err(Error::new(
            // 使用稳定错误分类区分能力缺口与资源故障。
            crate::core::Errc::NotImplemented,
            // 保留可定位的 Picture begin 诊断。
            "render target does not support Picture offscreen begin",
        ))
    }

    /// Checked Picture flush boundary. Unsupported targets fail explicitly.
    // 默认 target 不允许静默丢弃待提交的 Picture 绘制。
    fn try_flush_offscreen_paint(
        // 接收当前 render target 的唯一可变 owner。
        &mut self,
        // 接收必须提交的 Picture 目标身份。
        handle: &ImageHandle,
        // 返回 typed failure，阻止错误帧继续提交。
    ) -> Result<(), Error> {
        // 默认实现只消费参数，具体 target 必须显式覆盖提交语义。
        let _ = handle;
        // 缺失 Picture flush 属于明确的能力缺口。
        Err(Error::new(
            // 使用稳定错误分类驱动既有恢复策略。
            crate::core::Errc::NotImplemented,
            // 保留可定位的 Picture flush 诊断。
            "render target does not support Picture offscreen flush",
        ))
    }

    /// Checked Picture end boundary. Unsupported targets fail explicitly.
    // 默认 target 要求实现者显式闭合已经开始的 Picture 生命周期。
    fn try_end_offscreen_paint(
        // 接收当前 render target 的唯一可变 owner。
        &mut self,
        // 返回 typed failure，禁止绕过 target 恢复边界。
    ) -> Result<(), Error> {
        // 缺失 Picture end 属于明确的能力缺口。
        Err(Error::new(
            // 使用稳定错误分类驱动既有恢复策略。
            crate::core::Errc::NotImplemented,
            // 保留可定位的 Picture end 诊断。
            "render target does not support Picture offscreen end",
        ))
    }

    /// Checked ordered Picture blit boundary. Unsupported targets fail explicitly.
    // 默认 target 不允许把漏绘误报为成功帧。
    fn try_blit_offscreen_src(
        // 接收当前 render target 的唯一可变 owner。
        &mut self,
        // 接收稳定的 Picture 来源身份。
        handle: &ImageHandle,
        // 接收来源纹理内的裁剪区域。
        src_rect: Rect,
        // 接收当前目标内的绘制区域。
        dst_rect: Rect,
        // 返回 typed failure，阻止失败 blit 继续提交。
    ) -> Result<(), Error> {
        // 默认实现只消费参数，具体 target 必须显式覆盖合成语义。
        let _ = (handle, src_rect, dst_rect);
        // 缺失 Picture blit 属于明确的能力缺口。
        Err(Error::new(
            // 使用稳定错误分类驱动既有恢复策略。
            crate::core::Errc::NotImplemented,
            // 保留可定位的 Picture blit 诊断。
            "render target does not support Picture offscreen blit",
        ))
    }

    /// 对 Picture 离屏目标做可分离高斯模糊。
    ///
    /// GPU 路径须走原生 RT 模糊（`RenderBackend::try_blur_offscreen`），禁止
    /// PixelUpload 冒充；半径语义同 CPU `gaussian_blur`（`sigma = radius / 3`）。
    fn try_blur_offscreen(
        &mut self,
        _handle: &ImageHandle,
        _region: Rect,
        _radius: f32,
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::Errc::NotImplemented,
            "render target does not support offscreen separable blur",
        ))
    }

    fn memory_usage(&self) -> usize {
        0
    }
    fn diagnose_memory(&self) {}
}
