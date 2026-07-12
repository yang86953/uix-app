//! 2D 绘制上下文协议。

use crate::core::{Rect, Size};
use crate::draw::primitives::color::Color;
use crate::draw::primitives::path::{FillRule, Path};
use crate::draw::primitives::stroker::StrokeOptions;
use crate::draw::primitives::types::{BlendMode, GradientDirection, Radius};

pub trait Canvas2D {
    // ── 画布偏移（像素空间平移）──

    /// 当前像素偏移量（影响所有绘制操作的坐标）。
    fn offset(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// 设置像素偏移量。
    fn set_offset(&mut self, _dx: f32, _dy: f32) {}

    /// 累加像素偏移。
    fn translate(&mut self, dx: f32, dy: f32) {
        let (ox, oy) = self.offset();
        self.set_offset(ox + dx, oy + dy);
    }

    // ── 矢量填充（均有默认实现委托 Rasterizer）──
    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::fill::fill_rect(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h),
            color,
            radius,
        );
    }
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::fill::fill_circle(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            cx + ox,
            cy + oy,
            r,
            color,
        );
    }
    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::fill::fill_ellipse(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h),
            color,
        );
    }
    fn fill_sector(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
    ) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::fill::fill_sector(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            cx + ox,
            cy + oy,
            r,
            start_angle,
            end_angle,
            color,
        );
    }
    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        let (ox, oy) = self.offset();
        let path = if ox != 0.0 || oy != 0.0 {
            std::borrow::Cow::Owned(path.translated(ox, oy))
        } else {
            std::borrow::Cow::Borrowed(path)
        };
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::fill::fill_path(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            &path,
            color,
            fill_rule,
        );
    }

    // ── 矢量描边（均有默认实现）──
    fn stroke_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Option<Radius>) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::stroke::stroke_rect(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h),
            color,
            line_width,
            radius,
        );
    }
    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::stroke::stroke_circle(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            cx + ox,
            cy + oy,
            r,
            color,
            line_width,
        );
    }
    fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        let (ox, oy) = self.offset();
        let path = if ox != 0.0 || oy != 0.0 {
            std::borrow::Cow::Owned(path.translated(ox, oy))
        } else {
            std::borrow::Cow::Borrowed(path)
        };
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::stroke::stroke_path(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            &path,
            color,
            opts,
        );
    }
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::stroke::draw_line(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            x1 + ox,
            y1 + oy,
            x2 + ox,
            y2 + oy,
            color,
            width,
        );
    }

    // ── 渐变（均有默认实现）──
    fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
    ) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::gradient::fill_linear_gradient(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h),
            color_a,
            color_b,
            dir,
        );
    }
    fn fill_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        inner_r: f32,
        outer_r: f32,
        inner_color: Color,
        outer_color: Color,
    ) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::gradient::fill_radial_gradient(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            cx + ox,
            cy + oy,
            inner_r,
            outer_r,
            inner_color,
            outer_color,
        );
    }

    // ── 阴影（均有默认实现）──
    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::shadow::draw_box_shadow(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h),
            blur_radius,
            offset_x,
            offset_y,
            color,
            corner_radius,
        );
    }
    fn draw_box_shadow_ambient(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::shadow::draw_box_shadow_ambient(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h),
            blur_radius,
            offset_x,
            offset_y,
            color,
            corner_radius,
        );
    }

    // ── 图像/字形混合（均有默认实现）──
    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::image::blit_image(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            src,
            src_w,
            src_rect,
            Rect::new(dst_rect.x + ox, dst_rect.y + oy, dst_rect.w, dst_rect.h),
        );
    }
    fn blit_glyph(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        width: usize,
        height: usize,
        color: Color,
    ) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::draw::rasterizer::glyph::blit_glyph(
            pixels,
            size.w as i32,
            size.h as i32,
            clip,
            opacity,
            (x as f32 + ox) as i32,
            (y as f32 + oy) as i32,
            coverage,
            width,
            height,
            color,
        );
    }

    /// Shared-coverage glyph blit — default copies via [`blit_glyph`]; GPU-native
    /// backends may retain the `Arc` without cloning coverage bytes (#105).
    fn blit_glyph_shared(
        &mut self,
        x: i32,
        y: i32,
        coverage: std::sync::Arc<[u8]>,
        width: usize,
        height: usize,
        color: Color,
    ) {
        self.blit_glyph(x, y, coverage.as_ref(), width, height, color);
    }

    // ── 渲染状态栈（必须自行实现）──
    fn save(&mut self);
    fn restore(&mut self);
    fn push_clip(&mut self, rect: Rect);
    fn pop_clip(&mut self);
    fn set_opacity(&mut self, opacity: f32);
    fn opacity(&self) -> f32;
    fn set_blend_mode(&mut self, mode: BlendMode);
    fn push_clip_path(&mut self, path: &Path);

    // ── 像素访问（必须自行实现）──
    fn pixels_mut(&mut self) -> &mut [u32];
    fn surface_size(&self) -> Size;
    fn width(&self) -> i32 {
        self.surface_size().w as i32
    }
    fn height(&self) -> i32 {
        self.surface_size().h as i32
    }
    fn current_clip(&self) -> Rect;

    // ── 像素移动（滚动优化）──

    /// 在画布上移动一个矩形区域内的像素（scroll/pan 优化）。
    /// 将 viewport 区域内的像素从 `(viewport.x-dx, viewport.y-dy)` 复制到
    /// `(viewport.x, viewport.y)`，避免全帧重绘。dx/dy 应是整数像素偏移。
    /// 调用此方法后，viewport 中非 strip 区域的内容已正确偏移，
    /// 调用者只需重绘新暴露的 strip 区域（与滚动方向相反的一侧）。
    fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32);
}
