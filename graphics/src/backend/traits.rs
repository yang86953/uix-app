//! 渲染后端 trait — 仅负责 surface 与像素提交，不含帧调度逻辑。

use std::any::Any;

use uix_platform::{Error, Point, Rect, Size};

use crate::api::types::ImageHandle;
use crate::traits::{Canvas2D, PresentationMode};

/// 后端种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// CPU 软件光栅化。
    Cpu,
    /// GPU 加速（需平台图形上下文）。
    Gpu,
    /// 自动选择：当前等价于 Cpu。
    Auto,
    /// 空后端，用于测试。
    Null,
}

/// 呈现损伤区域（Phase 8 扩展为多矩形）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DamageRegion {
    /// 合并后的 bounds；`None` 表示全屏。
    pub bounds: Option<Rect>,
}

impl DamageRegion {
    pub fn full() -> Self {
        Self { bounds: None }
    }

    pub fn from_rect(rect: Rect) -> Self {
        Self {
            bounds: Some(rect),
        }
    }
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

impl From<BackendCapabilities> for crate::traits::GraphicsCapabilities {
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
