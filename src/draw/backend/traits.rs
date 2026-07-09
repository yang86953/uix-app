//! 渲染后端 trait — 仅负责 surface 与像素提交，不含帧调度逻辑。

use std::any::Any;

use crate::core::{DamageRegion, Error, Point, Rect, Size};

use crate::draw::traits::{Canvas2D, PresentationMode};
use crate::draw::ImageHandle;

/// Engine-level raster preference ([#169](docs/decisions.md#d169)).
///
/// Aligned with orthogonal axes: `Cpu` / `Gpu` select raster path preference;
/// concrete API is [`crate::native::traits::GraphicsBackend`]. Not a bundled
/// pipeline enum — see `RasterMode` × `PresentMode` × `GraphicsBackend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Prefer CPU software raster (`RasterMode::Cpu`).
    Cpu,
    /// Prefer GPU path (`RasterMode::GpuNative` when context supports it).
    Gpu,
    /// Auto: without a platform GPU context, equivalent to Cpu; app bootstrap
    /// owns GPU-first fallback via `bootstrap_graphics_engine`.
    Auto,
    /// Null backend for tests.
    Null,
}

/// 后端能力声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub presentation_mode: PresentationMode,
    pub partial_redraw: bool,
    pub offscreen: bool,
    pub scroll_memmove: bool,
}

impl BackendCapabilities {
    pub fn cpu() -> Self {
        Self {
            presentation_mode: PresentationMode::ExternalPresenter,
            partial_redraw: true,
            offscreen: true,
            scroll_memmove: true,
        }
    }

    /// GPU 默认能力：支持局部 clear 与 partial present damage。
    pub fn gpu() -> Self {
        Self {
            presentation_mode: PresentationMode::EngineManaged,
            partial_redraw: true,
            offscreen: false,
            scroll_memmove: false,
        }
    }

    /// GPU 降级：不支持 partial，pipeline 会扩为全帧重绘。
    pub fn gpu_full_redraw() -> Self {
        Self {
            presentation_mode: PresentationMode::EngineManaged,
            partial_redraw: false,
            offscreen: false,
            scroll_memmove: false,
        }
    }

    pub fn null() -> Self {
        Self {
            presentation_mode: PresentationMode::ExternalPresenter,
            partial_redraw: true,
            offscreen: false,
            scroll_memmove: false,
        }
    }
}

impl From<BackendCapabilities> for crate::draw::traits::GraphicsCapabilities {
    fn from(caps: BackendCapabilities) -> Self {
        Self {
            presentation_mode: caps.presentation_mode,
            partial_redraw: caps.partial_redraw,
        }
    }
}

/// 可绘制 surface — Backend 提供，Pipeline 通过此接口写入。
pub trait DrawSurface: Send {
    fn size(&self) -> Size;
    fn width(&self) -> i32 {
        self.size().w as i32
    }
    fn height(&self) -> i32 {
        self.size().h as i32
    }

    fn push_clip(&mut self, rect: Rect);
    fn pop_clip(&mut self);

    fn clear_all(&mut self);
    fn clear_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32);

    fn copy_region(&mut self, src: Rect, dst: Point);

    fn canvas(&mut self) -> &mut dyn Canvas2D;
}

/// 渲染后端 — 只负责 surface 与像素提交。
pub trait RenderBackend: Send {
    fn kind(&self) -> BackendKind;
    fn capabilities(&self) -> BackendCapabilities;

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
    fn shutdown(&mut self);

    fn surface(&mut self) -> &mut dyn DrawSurface;

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

    fn present(&mut self, damage: &DamageRegion) -> Result<(), Error> {
        let _ = damage;
        Ok(())
    }

    /// 用于具体后端类型的向下转型（如 GpuEngine 读回像素）。
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
