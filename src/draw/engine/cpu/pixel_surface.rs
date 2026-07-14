//! 纯像素表面——只持有像素缓冲和尺寸。
//!
//! 实现 RenderingBackend，供 CpuCanvas2D 底层使用。

use crate::core::{Rect, Size};

use crate::draw::primitives::color::Color;
use crate::draw::traits::RenderingBackend;

/// CPU 像素表面。
///
/// 不持有任何渲染状态（clip/opacity/transform 等），
/// 状态管理全部在 CpuCanvas2D 中。
pub struct PixelSurface {
    pixels: Vec<u32>,
    width: i32,
    height: i32,
    clear_color: Color,
}

impl PixelSurface {
    /// 创建指定尺寸的像素表面。
    pub fn new(width: i32, height: i32) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        Self {
            // Transparent clear: soft-fallback buffers are alpha-blitted over
            // GPU-native content; opaque black would wipe the frame (#105).
            pixels: vec![0x00000000; (w * h) as usize],
            width: w,
            height: h,
            clear_color: Color::transparent(),
        }
    }

    /// 设置清除时使用的颜色。
    pub fn set_clear_color(&mut self, color: Color) {
        self.clear_color = color;
    }

    /// 当前清除颜色。
    pub fn clear_color(&self) -> Color {
        self.clear_color
    }

    /// 获取表面宽度。
    pub fn width(&self) -> i32 {
        self.width
    }

    /// 获取表面高度。
    pub fn height(&self) -> i32 {
        self.height
    }

    /// 表面尺寸。
    pub fn surface_size(&self) -> Size {
        Size::new(self.width as f32, self.height as f32)
    }

    /// 可变像素切片。
    pub fn pixels_mut(&mut self) -> &mut [u32] {
        &mut self.pixels
    }

    /// 只读像素切片。
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// 清空整个表面。
    pub fn clear_all(&mut self) {
        // All CPU raster pixels are premultiplied AARRGGBB. A straight-alpha
        // clear color would poison subsequent source-over blends, especially
        // when a Picture cache is cleared to a translucent color.
        let c = self.clear_color.premultiplied();
        self.pixels.fill(c);
    }

    /// 清除指定矩形区域。
    pub fn clear_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let c = self.clear_color.premultiplied();
        let surf_w = self.width;
        let surf_h = self.height;

        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(surf_w);
        let y1 = (y + h).min(surf_h);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        for row in y0..y1 {
            let start = (row * surf_w + x0) as usize;
            let end = (row * surf_w + x1) as usize;
            self.pixels[start..end].fill(c);
        }
    }

    /// 像素复制（memmove 语义，允许源与目标重叠）。
    /// `pixels` 参数必须与 self.pixels 指向同一缓冲（由 &self 限制）。
    pub fn copy_region(&self, src_rect: Rect, dst_x: i32, dst_y: i32, pixels: &mut [u32]) {
        if src_rect.w <= 0.0 || src_rect.h <= 0.0 {
            return;
        }
        let src_x = src_rect.x as i32;
        let src_y = src_rect.y as i32;
        let copy_w = src_rect.w as i32;
        let copy_h = src_rect.h as i32;

        let surf_w = self.width;
        let surf_h = self.height;

        let clip_x0 = src_x.max(0).max(src_x - dst_x);
        let clip_y0 = src_y.max(0).max(src_y - dst_y);
        let clip_x1 = (src_x + copy_w).min(surf_w).min(surf_w + src_x - dst_x);
        let clip_y1 = (src_y + copy_h).min(surf_h).min(surf_h + src_y - dst_y);

        if clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
            return;
        }

        let row_len = (clip_x1 - clip_x0) as usize;

        if dst_y <= src_y {
            for row in clip_y0..clip_y1 {
                let src_idx = (row * surf_w + clip_x0) as usize;
                let dst_idx = ((row + dst_y - src_y) * surf_w + (clip_x0 + dst_x - src_x)) as usize;
                pixels.copy_within(src_idx..src_idx + row_len, dst_idx);
            }
        } else {
            for row in (clip_y0..clip_y1).rev() {
                let src_idx = (row * surf_w + clip_x0) as usize;
                let dst_idx = ((row + dst_y - src_y) * surf_w + (clip_x0 + dst_x - src_x)) as usize;
                pixels.copy_within(src_idx..src_idx + row_len, dst_idx);
            }
        }
    }
}

impl RenderingBackend for PixelSurface {
    fn surface_size(&self) -> Size {
        Size::new(self.width as f32, self.height as f32)
    }

    fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    fn pixels_mut(&mut self) -> &mut [u32] {
        &mut self.pixels
    }

    fn clear_rect(&mut self, rect: Rect, color: Color) {
        let saved = self.clear_color;
        self.clear_color = color;
        self.clear_rect_raw(rect.x as i32, rect.y as i32, rect.w as i32, rect.h as i32);
        self.clear_color = saved;
    }

    fn present(&mut self) {
        // CPU 不需要呈现操作，像素已经在内存中。
    }

    fn copy_region(&mut self, src: Rect, dst_x: i32, dst_y: i32) {
        // 由于 borrow checker 限制，直接使用 pixels.copy_within
        if src.w <= 0.0 || src.h <= 0.0 {
            return;
        }
        let src_x = src.x as i32;
        let src_y = src.y as i32;
        let copy_w = src.w as i32;
        let copy_h = src.h as i32;
        let surf_w = self.width;
        let surf_h = self.height;

        let clip_x0 = src_x.max(0).max(src_x - dst_x);
        let clip_y0 = src_y.max(0).max(src_y - dst_y);
        let clip_x1 = (src_x + copy_w).min(surf_w).min(surf_w + src_x - dst_x);
        let clip_y1 = (src_y + copy_h).min(surf_h).min(surf_h + src_y - dst_y);

        if clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
            return;
        }

        let row_len = (clip_x1 - clip_x0) as usize;

        if dst_y <= src_y {
            for row in clip_y0..clip_y1 {
                let src_idx = (row * surf_w + clip_x0) as usize;
                let dst_idx = ((row + dst_y - src_y) * surf_w + (clip_x0 + dst_x - src_x)) as usize;
                self.pixels.copy_within(src_idx..src_idx + row_len, dst_idx);
            }
        } else {
            for row in (clip_y0..clip_y1).rev() {
                let src_idx = (row * surf_w + clip_x0) as usize;
                let dst_idx = ((row + dst_y - src_y) * surf_w + (clip_x0 + dst_x - src_x)) as usize;
                self.pixels.copy_within(src_idx..src_idx + row_len, dst_idx);
            }
        }
    }
}
