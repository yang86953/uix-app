//! 渲染引擎协议 — 帧生命周期、更新策略与引擎能力。

pub use crate::draw::engine::RenderOutcome;

use super::canvas::Canvas2D;
use crate::core::{Error, Rect};
use crate::draw::pipeline::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::primitives::types::ImageHandle;

/// 帧更新策略。
#[derive(Debug, Clone)]
pub enum UpdateStrategy {
    FullRedraw,
    DirtyRects(Vec<Rect>),
}

impl UpdateStrategy {
    pub fn rects(&self) -> Option<&[Rect]> {
        match self {
            UpdateStrategy::FullRedraw => None,
            UpdateStrategy::DirtyRects(rects) => Some(rects),
        }
    }

    pub fn should_clear(&self) -> bool {
        matches!(
            self,
            UpdateStrategy::FullRedraw | UpdateStrategy::DirtyRects(_)
        )
    }
}

/// 帧最终呈现方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationMode {
    /// 引擎输出 CPU 像素缓冲，由 platform presenter 提交到窗口。
    ExternalPresenter,
    /// 引擎内部已完成呈现，例如 GPU 后端自行 swap buffers。
    EngineManaged,
}

/// 图形引擎能力声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsCapabilities {
    pub presentation_mode: PresentationMode,
    pub partial_redraw: bool,
    /// 是否支持 Picture 离屏缓存（`create_offscreen` / blit）。
    pub offscreen: bool,
}

impl GraphicsCapabilities {
    pub fn cpu_pixels() -> Self {
        Self {
            presentation_mode: PresentationMode::ExternalPresenter,
            partial_redraw: true,
            offscreen: true,
        }
    }

    pub fn engine_managed_full_redraw() -> Self {
        Self {
            presentation_mode: PresentationMode::EngineManaged,
            partial_redraw: false,
            offscreen: false,
        }
    }

    /// Engine-managed present + CPU 离屏（PresentUpload 等）。
    pub fn engine_managed_with_offscreen() -> Self {
        Self {
            presentation_mode: PresentationMode::EngineManaged,
            partial_redraw: false,
            offscreen: true,
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
}

/// 图形引擎 — 帧生命周期与离屏缓冲管理。
pub trait GraphicsEngine: 'static {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error>;
    fn shutdown(&mut self);
    /// Checked counterpart of [`Self::shutdown`]. The default retains legacy
    /// CPU/test engines while native engines can preserve a failed teardown
    /// for recovery instead of reducing it to a log-only event.
    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.shutdown();
        Ok(())
    }
    /// Resize is a graphics lifecycle operation and must propagate a typed
    /// failure. Callers retain invalidation and enter bounded recovery.
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome;
    /// `present_damage` 为合成层计算的呈现损伤；EngineManaged 后端用于 swap/present。
    fn end_frame(&mut self, present_damage: &crate::draw::backend::DamageRegion) -> RenderOutcome;
    fn canvas_2d(&mut self) -> &mut dyn Canvas2D;

    /// The sole external platform presenter has accepted the pending frame.
    /// Engine-managed paths must never receive this callback.
    fn external_present_succeeded(&mut self) {}

    /// The external platform presenter rejected a pending frame. The default
    /// is intentionally inert; recovery wrappers record this typed failure at
    /// the next frame boundary while ordinary software engines retain dirty.
    fn external_present_failed(&mut self, _error: Error) {}

    fn capabilities(&self) -> GraphicsCapabilities {
        GraphicsCapabilities::cpu_pixels()
    }

    fn dpi(&self) -> f32 {
        96.0
    }
    fn device_pixel_ratio(&self) -> f32 {
        1.0
    }
    fn orientation(&self) -> crate::draw::spatial::Orientation {
        crate::draw::spatial::Orientation::YDown
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
    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        let _ = (handle, dst_rect);
    }
    fn blit_offscreen_src(&mut self, _handle: &ImageHandle, _src_rect: Rect, _dst_rect: Rect) {}
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
    /// `Unsupported`; FrameRenderer deliberately treats that as a typed frame
    /// failure rather than resuming a direct backend-canvas path.
    fn try_execute_encoded_frame(
        &mut self,
        _encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        Ok(EncodedFrameExecution::Unsupported)
    }

    /// Bind/clear offscreen before Picture paint (GPU RT or CPU buffer).
    fn begin_offscreen_paint(&mut self, handle: &ImageHandle) -> bool {
        let _ = handle;
        true
    }

    /// Checked Picture offscreen boundary.  Implementations backed by native
    /// APIs override this to propagate bind/clear failures to FrameRenderer;
    /// the legacy bool method remains for compatibility with old callers.
    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        if self.begin_offscreen_paint(handle) {
            Ok(())
        } else {
            Err(Error::new(
                crate::core::Errc::InvalidState,
                "graphics engine could not begin Picture offscreen paint",
            ))
        }
    }

    fn flush_offscreen_paint(&mut self, handle: &ImageHandle) {
        let _ = handle;
    }

    /// Checked counterpart of [`Self::flush_offscreen_paint`].
    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.flush_offscreen_paint(handle);
        Ok(())
    }

    fn end_offscreen_paint(&mut self) {}

    /// Checked counterpart of [`Self::end_offscreen_paint`].
    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.end_offscreen_paint();
        Ok(())
    }

    /// Checked ordered Picture blit boundary.  The default preserves legacy
    /// engines; native implementations must override it when their blit can
    /// fail.
    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        self.blit_offscreen_src(handle, src_rect, dst_rect);
        Ok(())
    }

    fn memory_usage(&self) -> usize {
        0
    }
    fn diagnose_memory(&self) {}
}
