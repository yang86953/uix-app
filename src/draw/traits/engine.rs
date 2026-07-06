//! 渲染引擎协议 — 帧生命周期、更新策略与引擎能力。

pub use crate::draw::engine::RenderOutcome;

use super::canvas::Canvas2D;
use crate::core::{Error, Rect};
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
}

impl GraphicsCapabilities {
    pub fn cpu_pixels() -> Self {
        Self {
            presentation_mode: PresentationMode::ExternalPresenter,
            partial_redraw: true,
        }
    }

    pub fn engine_managed_full_redraw() -> Self {
        Self {
            presentation_mode: PresentationMode::EngineManaged,
            partial_redraw: false,
        }
    }

    pub fn uses_external_presenter(self) -> bool {
        matches!(self.presentation_mode, PresentationMode::ExternalPresenter)
    }

    pub fn supports_partial_redraw(self) -> bool {
        self.partial_redraw
    }
}

/// 图形引擎 — 帧生命周期与离屏缓冲管理。
pub trait GraphicsEngine: 'static {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error>;
    fn shutdown(&mut self);
    fn resize(&mut self, width: i32, height: i32);
    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome;
    /// `present_damage` 为合成层计算的呈现损伤；EngineManaged 后端用于 swap/present。
    fn end_frame(&mut self, present_damage: &crate::draw::backend::DamageRegion) -> RenderOutcome;
    fn canvas_2d(&mut self) -> &mut dyn Canvas2D;

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

    fn memory_usage(&self) -> usize {
        0
    }
    fn diagnose_memory(&self) {}
}
