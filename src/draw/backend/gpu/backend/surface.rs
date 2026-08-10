//! [`NativeGpuDrawSurface`]：`DrawSurface` 门面 — backend 子模块。
//!
//! 把 canvas 的软/硬清除与绘制转发到 [`NativeGpuCanvas2D`]，帧边界错误
//! 通过 deferred error 消费。

use crate::core::{Error, Point, Rect};
use crate::draw::Canvas2D;
use crate::draw::backend::contract::DrawSurface;
// 引入同一 graphics backend Module 的 canvas owner。
use super::super::canvas::NativeGpuCanvas2D;
// 引入同一 graphics backend Module 的纯色矩形原语。
use super::super::GpuSolidRect;
// 引入同一 Module 统一导出的能力投影类型。
use super::super::NativeRasterCaps;

pub struct NativeGpuDrawSurface {
    pub(crate) canvas: NativeGpuCanvas2D,
    pub(crate) native_caps: NativeRasterCaps,
    pub(crate) width: i32,
    pub(crate) height: i32,
    /// Full clear pending (ClearRenderTargetView).
    pub(crate) needs_gpu_clear: bool,
    /// Partial clear rects (replace-blend quads).
    pub(crate) pending_clear_rects: Vec<GpuSolidRect>,
    /// Logical scroll copies waiting for the next retained RHI boundary.
    pub(crate) pending_scroll_copies: Vec<PendingScrollCopy>,
}

// 保存一次尚未落入 retained texture 的逻辑像素搬移。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PendingScrollCopy {
    // 保存与 CPU surface 相同的源矩形。
    pub(crate) source: Rect,
    // 保存源矩形搬移后的目标左上角。
    pub(crate) destination: Point,
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
        self.pending_scroll_copies.clear();
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
        // 局部清理由通用 FramePlan 记录并交给 retained RHI 执行。
        self.pending_clear_rects.push(GpuSolidRect {
            x: x as f32,
            y: y as f32,
            w: w as f32,
            h: h as f32,
            rgba: [0.0, 0.0, 0.0, 0.0],
            radius: [0.0; 4],
        });
    }

    fn copy_region(&mut self, src: Rect, dst: Point) {
        // 记录 copy_region，后续由 retained RHI 的 TextureMove 提供 memmove 语义。
        self.pending_scroll_copies.push(PendingScrollCopy {
            source: src,
            destination: dst,
        });
    }

    fn canvas(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    fn take_deferred_error(&mut self) -> Option<Error> {
        self.canvas.take_deferred_error()
    }
}
