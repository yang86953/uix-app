//! [`NativeGpuDrawSurface`]：`DrawSurface` 门面 — backend 子模块。
//!
//! 把 canvas 的软/硬清除与绘制转发到 [`NativeGpuCanvas2D`]，帧边界错误
//! 通过 deferred error 消费。

use crate::core::{Error, Point, PresentDamageTracker, Rect};
use crate::draw::backend::contract::DrawSurface;
use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::{ImageHandle, Radius};
use crate::draw::painting::FrameRect;
use crate::draw::Canvas2D;
use crate::native::present::{GpuSolidRect, NativeRasterCaps};

use super::super::canvas::NativeGpuCanvas2D;

pub struct NativeGpuDrawSurface {
    pub(crate) canvas: NativeGpuCanvas2D,
    pub(crate) native_caps: NativeRasterCaps,
    pub(crate) width: i32,
    pub(crate) height: i32,
    /// Full clear pending (ClearRenderTargetView).
    pub(crate) needs_gpu_clear: bool,
    /// Partial clear rects (replace-blend quads).
    pub(crate) pending_clear_rects: Vec<GpuSolidRect>,
}

impl DrawSurface for NativeGpuDrawSurface {

    fn size(&self) -> crate::core::Size {
        crate::core::Size::new(self.width as f32, self.height as f32)
    }

    fn push_clip(&mut self, rect: Rect) {
        self.canvas.push_clip(rect);
    }

    fn pop_clip(&mut self) {
        self.canvas.pop_clip();
    }

    fn clear_all(&mut self) {
        self.canvas.clear_soft();
        self.pending_clear_rects.clear();
        self.needs_gpu_clear = true;
    }

    fn clear_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.canvas.clear_soft_rect(x, y, w, h);
        if self.needs_gpu_clear {
            return;
        }
        if !self.native_caps.clear_rects {
            self.pending_clear_rects.clear();
            self.needs_gpu_clear = true;
            return;
        }
        self.pending_clear_rects.push(GpuSolidRect {
            x: x as f32,
            y: y as f32,
            w: w as f32,
            h: h as f32,
            rgba: [0.0, 0.0, 0.0, 0.0],
            radius: [0.0; 4],
        });
    }

    fn copy_region(&mut self, _src: Rect, _dst: Point) {
        self.canvas.reject_unsupported("scroll-region copy");
    }

    fn canvas(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    fn take_deferred_error(&mut self) -> Option<Error> {
        self.canvas.take_deferred_error()
    }
}
