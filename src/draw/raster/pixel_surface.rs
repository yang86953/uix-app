//! 纯像素表面——只持有像素缓冲和尺寸。
//!
//! 实现 RenderingBackend，供 CpuCanvas2D 底层使用。

use crate::core::{Errc, Error, Rect, Size};

use crate::draw::geometry::color::Color;

/// CPU 像素表面。
///
/// 不持有任何渲染状态（clip/opacity/transform 等），
/// 状态管理全部在 CpuCanvas2D 中。
pub(crate) struct PixelSurface {
    pixels: Vec<u32>,
    width: i32,
    height: i32,
    clear_color: Color,
}

#[derive(Clone, Copy)]
struct ClippedCopyRegion {
    src_x: usize,
    src_y: usize,
    dst_x: usize,
    dst_y: usize,
    width: usize,
    height: usize,
}

fn clipped_copy_region(
    surface_width: i32,
    surface_height: i32,
    src: Rect,
    dst_x: i32,
    dst_y: i32,
) -> Option<ClippedCopyRegion> {
    if !src.x.is_finite()
        || !src.y.is_finite()
        || !src.w.is_finite()
        || !src.h.is_finite()
        || src.w <= 0.0
        || src.h <= 0.0
    {
        return None;
    }

    // 裁剪加减法先扩宽；widget 坐标从 f32 转换后可能合法地饱和到 i32 边界。
    let src_x = (src.x as i32) as i64;
    let src_y = (src.y as i32) as i64;
    let copy_width = (src.w as i32) as i64;
    let copy_height = (src.h as i32) as i64;
    if copy_width <= 0 || copy_height <= 0 {
        return None;
    }

    let surface_width = surface_width as i64;
    let surface_height = surface_height as i64;
    let delta_x = dst_x as i64 - src_x;
    let delta_y = dst_y as i64 - src_y;
    let x0 = src_x.max(0).max(-delta_x);
    let y0 = src_y.max(0).max(-delta_y);
    let x1 = (src_x + copy_width)
        .min(surface_width)
        .min(surface_width - delta_x);
    let y1 = (src_y + copy_height)
        .min(surface_height)
        .min(surface_height - delta_y);
    if x0 >= x1 || y0 >= y1 {
        return None;
    }

    Some(ClippedCopyRegion {
        src_x: x0 as usize,
        src_y: y0 as usize,
        dst_x: (x0 + delta_x) as usize,
        dst_y: (y0 + delta_y) as usize,
        width: (x1 - x0) as usize,
        height: (y1 - y0) as usize,
    })
}

fn copy_region_within(
    pixels: &mut [u32],
    surface_width: i32,
    surface_height: i32,
    src: Rect,
    dst_x: i32,
    dst_y: i32,
) {
    let Some(region) = clipped_copy_region(surface_width, surface_height, src, dst_x, dst_y) else {
        return;
    };
    let row_stride = surface_width as usize;
    let expected_len = row_stride.saturating_mul(surface_height as usize);
    if pixels.len() < expected_len {
        return;
    }

    if region.dst_y <= region.src_y {
        for row_offset in 0..region.height {
            let src_idx = (region.src_y + row_offset) * row_stride + region.src_x;
            let dst_idx = (region.dst_y + row_offset) * row_stride + region.dst_x;
            pixels.copy_within(src_idx..src_idx + region.width, dst_idx);
        }
    } else {
        for row_offset in (0..region.height).rev() {
            let src_idx = (region.src_y + row_offset) * row_stride + region.src_x;
            let dst_idx = (region.dst_y + row_offset) * row_stride + region.dst_x;
            pixels.copy_within(src_idx..src_idx + region.width, dst_idx);
        }
    }
}

impl PixelSurface {
    fn checked_extent(width: i32, height: i32) -> Result<(i32, i32, usize), Error> {
        let w = width.max(1);
        let h = height.max(1);
        let pixel_count = (w as usize).checked_mul(h as usize).ok_or_else(|| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!("PixelSurface extent {w}x{h} exceeds addressable memory"),
            )
        })?;
        if pixel_count > (isize::MAX as usize) / std::mem::size_of::<u32>() {
            return Err(Error::new(
                Errc::GraphicsOutOfMemory,
                format!("PixelSurface extent {w}x{h} exceeds addressable memory"),
            ));
        }
        Ok((w, h, pixel_count))
    }

    /// Validates that an extent is addressable without reserving its pixels.
    pub(crate) fn validate_extent(width: i32, height: i32) -> Result<(), Error> {
        Self::checked_extent(width, height).map(|_| ())
    }

    /// 创建指定尺寸的像素表面。
    ///
    /// # Panics
    ///
    /// 尺寸无法分配时 panic；运行时尺寸应使用 [`Self::try_new`] 接收 typed OOM。
    pub(crate) fn new(width: i32, height: i32) -> Self {
        match Self::try_new(width, height) {
            Ok(surface) => surface,
            Err(error) => panic!("PixelSurface::new failed: {error}"),
        }
    }

    /// 创建指定尺寸的像素表面，并把容量溢出或分配失败转换为 typed OOM。
    pub(crate) fn try_new(width: i32, height: i32) -> Result<Self, Error> {
        let (w, h, pixel_count) = Self::checked_extent(width, height)?;
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(pixel_count).map_err(|error| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!("PixelSurface allocation for {w}x{h} pixels failed: {error}"),
            )
        })?;
        pixels.resize(pixel_count, 0x00000000);
        Ok(Self {
            // Transparent clear: soft-fallback buffers are alpha-blitted over
            // GPU-native content; opaque black would wipe the frame (#105).
            pixels,
            width: w,
            height: h,
            clear_color: Color::transparent(),
        })
    }

    pub(crate) fn one_pixel() -> Self {
        Self {
            pixels: vec![0x00000000],
            width: 1,
            height: 1,
            clear_color: Color::transparent(),
        }
    }

    /// 设置清除时使用的颜色。
    pub(crate) fn set_clear_color(&mut self, color: Color) {
        self.clear_color = color;
    }

    /// 获取表面宽度。
    pub(crate) fn width(&self) -> i32 {
        self.width
    }

    /// 获取表面高度。
    pub(crate) fn height(&self) -> i32 {
        self.height
    }

    /// 表面尺寸。
    pub(crate) fn surface_size(&self) -> Size {
        Size::new(self.width as f32, self.height as f32)
    }

    /// 可变像素切片。
    pub(crate) fn pixels_mut(&mut self) -> &mut [u32] {
        &mut self.pixels
    }

    /// 只读像素切片。
    pub(crate) fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    pub(crate) fn memory_usage(&self) -> usize {
        self.pixels.len().saturating_mul(std::mem::size_of::<u32>())
    }

    /// 清空整个表面。
    pub(crate) fn clear_all(&mut self) {
        // All CPU raster pixels are premultiplied AARRGGBB. A straight-alpha
        // clear color would poison subsequent source-over blends, especially
        // when a Picture cache is cleared to a translucent color.
        let c = self.clear_color.premultiplied();
        self.pixels.fill(c);
    }

    /// 清除指定矩形区域。
    pub(crate) fn clear_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let c = self.clear_color.premultiplied();
        let surf_w = self.width;
        let surf_h = self.height;

        let x0 = (x as i64).clamp(0, surf_w as i64) as usize;
        let y0 = (y as i64).clamp(0, surf_h as i64) as usize;
        let x1 = (x as i64 + w as i64).clamp(0, surf_w as i64) as usize;
        let y1 = (y as i64 + h as i64).clamp(0, surf_h as i64) as usize;

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        let row_stride = surf_w as usize;
        for row in y0..y1 {
            let start = row * row_stride + x0;
            let end = row * row_stride + x1;
            self.pixels[start..end].fill(c);
        }
    }

    /// 像素复制（memmove 语义，允许源与目标重叠）。
    pub(crate) fn copy_region(&mut self, src_rect: Rect, dst_x: i32, dst_y: i32) {
        copy_region_within(
            &mut self.pixels,
            self.width,
            self.height,
            src_rect,
            dst_x,
            dst_y,
        );
    }
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../tests-src/draw/raster/pixel_surface_tests.rs"]
mod pixel_surface_tests;