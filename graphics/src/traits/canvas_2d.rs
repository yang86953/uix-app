//! 2D 绘制上下文——所有绘制原语 + 渲染状态。
//!
//! 全部绘制方法有默认实现（委托 Rasterizer 纯函数），
//! GPU 后端按需覆盖某些方法来走 shader 加速路径。
//! 渲染状态方法（save/restore/clip/opacity/transform/blend/pixels/size）必须自行实现。

use uix_platform::{Rect, Size};

use crate::color::Color;
use crate::path::{FillRule, Path};
use crate::rasterizer;
use crate::stroker::StrokeOptions;
use crate::types::{BlendMode, GradientDirection, Radius};

/// 2D 绘制能力接口。
///
/// 矢量填充、描边、渐变、阴影、图像/字形混合、渲染状态栈、画布偏移。
pub trait Canvas2D {
    // ═══════════════════════════════════════════
    // 画布偏移（像素空间平移）
    // ═══════════════════════════════════════════

    /// 当前像素偏移量（影响所有绘制操作的坐标）。
    /// 默认实现返回 (0.0, 0.0)。
    fn offset(&self) -> (f32, f32) { (0.0, 0.0) }

    /// 设置像素偏移量。
    /// 默认实现忽略。实现者应保存并应用 offset。
    fn set_offset(&mut self, dx: f32, dy: f32) { let _ = (dx, dy); }

    /// 累加像素偏移。
    fn translate(&mut self, dx: f32, dy: f32) {
        let (ox, oy) = self.offset();
        self.set_offset(ox + dx, oy + dy);
    }

    // ═══════════════════════════════════════════
    // 矢量填充（均有默认实现）
    // ═══════════════════════════════════════════

    /// 填充矩形，可选圆角。
    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        rasterizer::fill::fill_rect(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h), color, radius,
        );
    }

    /// 填充圆形。
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        rasterizer::fill::fill_circle(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            cx + ox, cy + oy, r, color,
        );
    }

    /// 填充椭圆。
    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        rasterizer::fill::fill_ellipse(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h), color,
        );
    }

    /// 填充扇形。
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
        rasterizer::fill::fill_sector(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            cx + ox, cy + oy, r, start_angle, end_angle, color,
        );
    }

    /// 填充闭合路径。
    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        let (ox, oy) = self.offset();
        let path = if ox != 0.0 || oy != 0.0 {
            // 路径所有坐标偏移
            std::borrow::Cow::Owned(path.translated(ox, oy))
        } else {
            std::borrow::Cow::Borrowed(path)
        };
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        rasterizer::fill::fill_path(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            &path, color, fill_rule,
        );
    }

    // ═══════════════════════════════════════════
    // 矢量描边（均有默认实现）
    // ═══════════════════════════════════════════

    /// 描边矩形，可选圆角。
    fn stroke_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Option<Radius>) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        rasterizer::stroke::stroke_rect(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h), color, line_width, radius,
        );
    }

    /// 描边圆形。
    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        rasterizer::stroke::stroke_circle(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            cx + ox, cy + oy, r, color, line_width,
        );
    }

    /// 描边路径。
    fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        let (ox, oy) = self.offset();
        let path = if ox != 0.0 || oy != 0.0 {
            std::borrow::Cow::Owned(path.translated(ox, oy))
        } else {
            std::borrow::Cow::Borrowed(path)
        };
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        rasterizer::stroke::stroke_path(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            &path, color, opts,
        );
    }

    /// 画直线。
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        rasterizer::stroke::draw_line(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            x1 + ox, y1 + oy, x2 + ox, y2 + oy, color, width,
        );
    }

    // ═══════════════════════════════════════════
    // 渐变（均有默认实现）
    // ═══════════════════════════════════════════

    /// 线性渐变填充。
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
        rasterizer::gradient::fill_linear_gradient(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h), color_a, color_b, dir,
        );
    }

    /// 径向渐变填充。
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
        rasterizer::gradient::fill_radial_gradient(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            cx + ox, cy + oy, inner_r, outer_r, inner_color, outer_color,
        );
    }

    // ═══════════════════════════════════════════
    // 阴影（均有默认实现）
    // ═══════════════════════════════════════════

    /// 盒阴影（定向模糊）。
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
        rasterizer::shadow::draw_box_shadow(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h),
            blur_radius, offset_x, offset_y, color, corner_radius,
        );
    }

    /// 环境阴影（更宽、更柔和的弥散）。
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
        rasterizer::shadow::draw_box_shadow_ambient(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h),
            blur_radius, offset_x, offset_y, color, corner_radius,
        );
    }

    // ═══════════════════════════════════════════
    // 图像/字形混合（均有默认实现）
    // ═══════════════════════════════════════════

    /// 将 src 矩形区域缩放绘制到 dst 矩形区域。
    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        let (ox, oy) = self.offset();
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        rasterizer::image::blit_image(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            src, src_w, src_rect,
            Rect::new(dst_rect.x + ox, dst_rect.y + oy, dst_rect.w, dst_rect.h),
        );
    }

    /// 将 coverage 位图和颜色混合到 (x, y) 位置。
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
        rasterizer::glyph::blit_glyph(
            pixels, size.w as i32, size.h as i32,
            clip, opacity,
            (x as f32 + ox) as i32, (y as f32 + oy) as i32,
            coverage, width, height, color,
        );
    }

    // ═══════════════════════════════════════════
    // 渲染状态栈（必须自行实现）
    // ═══════════════════════════════════════════

    /// 保存当前渲染状态（裁剪、透明度、偏移、变换、混合模式）。
    fn save(&mut self);

    /// 恢复上一次保存的渲染状态。
    fn restore(&mut self);

    /// 推入矩形裁剪区域（与当前裁剪区域取交集）。
    fn push_clip(&mut self, rect: Rect);

    /// 弹出矩形裁剪区域。
    fn pop_clip(&mut self);

    /// 设置全局透明度（0.0=全透明，1.0=完全不透明）。
    fn set_opacity(&mut self, opacity: f32);

    /// 获取当前全局透明度。
    fn opacity(&self) -> f32;



    // ═══════════════════════════════════════════
    // 混合模式
    // ═══════════════════════════════════════════

    /// 设置当前混合模式。默认 SrcOver。
    fn set_blend_mode(&mut self, mode: BlendMode);

    // ═══════════════════════════════════════════
    // 非矩形裁剪
    // ═══════════════════════════════════════════

    /// 推入 Path 裁剪区域（与当前裁剪区域取交集）。
    /// 用于圆形头像等非矩形裁剪场景。
    fn push_clip_path(&mut self, path: &Path);

    // ═══════════════════════════════════════════
    // 像素访问（必须自行实现）
    // ═══════════════════════════════════════════

    /// 可变像素缓冲。
    fn pixels_mut(&mut self) -> &mut [u32];

    /// 表面尺寸。
    fn surface_size(&self) -> Size;

    /// 表面宽度（像素）。
    fn width(&self) -> i32 {
        self.surface_size().w as i32
    }

    /// 表面高度（像素）。
    fn height(&self) -> i32 {
        self.surface_size().h as i32
    }

    /// 当前有效裁剪矩形。
    fn current_clip(&self) -> Rect;
}
