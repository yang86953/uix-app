//! # uix-graphics 行为契约
//!
//! 本模块定义 graphics 层的公开接口（trait）。
//! 各内部模块实现这些接口，外部使用者只依赖此处的接口签名。

use crate::api::types::{
    BlendMode, Color, FillRule, FontHandle, GlyphRaster, GradientDirection, ImageHandle,
    LineMetrics, Path, Radius, RenderOutcome, StrokeOptions, TextLayout,
};
use crate::text_backend::TextLayoutOptions;
use uix_platform::{Error, Rect, Size};

// ════════════════════════════════════════════════════════════════════════════
// 帧更新策略
// ════════════════════════════════════════════════════════════════════════════

/// 帧更新策略。
#[derive(Debug, Clone)]
pub enum UpdateStrategy {
    FullRedraw,
    DirtyRects(Vec<Rect>),
    Overlay(Vec<Rect>),
}

impl UpdateStrategy {
    pub fn rects(&self) -> Option<&[Rect]> {
        match self {
            UpdateStrategy::FullRedraw => None,
            UpdateStrategy::DirtyRects(rects) => Some(rects),
            UpdateStrategy::Overlay(rects) => Some(rects),
        }
    }

    pub fn should_clear(&self) -> bool {
        matches!(self, UpdateStrategy::FullRedraw | UpdateStrategy::DirtyRects(_))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 渲染后端
// ════════════════════════════════════════════════════════════════════════════

pub trait RenderingBackend {
    fn surface_size(&self) -> Size;
    fn pixels(&self) -> &[u32];
    fn pixels_mut(&mut self) -> &mut [u32];
    fn clear_rect(&mut self, rect: Rect, color: Color);
    fn present(&mut self);
    fn copy_region(&mut self, _src: Rect, _dst_x: i32, _dst_y: i32) {}
}

// ════════════════════════════════════════════════════════════════════════════
// 2D 绘制上下文
// ════════════════════════════════════════════════════════════════════════════

pub trait Canvas2D {
    // ── 矢量填充（均有默认实现委托 Rasterizer）──
    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::fill::fill_rect(pixels, size.w as i32, size.h as i32, clip, opacity, rect, color, radius);
    }
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::fill::fill_circle(pixels, size.w as i32, size.h as i32, clip, opacity, cx, cy, r, color);
    }
    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::fill::fill_ellipse(pixels, size.w as i32, size.h as i32, clip, opacity, rect, color);
    }
    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, start_angle: f32, end_angle: f32, color: Color) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::fill::fill_sector(pixels, size.w as i32, size.h as i32, clip, opacity, cx, cy, r, start_angle, end_angle, color);
    }
    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::fill::fill_path(pixels, size.w as i32, size.h as i32, clip, opacity, path, color, fill_rule);
    }

    // ── 矢量描边（均有默认实现）──
    fn stroke_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Option<Radius>) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::stroke::stroke_rect(pixels, size.w as i32, size.h as i32, clip, opacity, rect, color, line_width, radius);
    }
    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::stroke::stroke_circle(pixels, size.w as i32, size.h as i32, clip, opacity, cx, cy, r, color, line_width);
    }
    fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::stroke::stroke_path(pixels, size.w as i32, size.h as i32, clip, opacity, path, color, opts);
    }
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::stroke::draw_line(pixels, size.w as i32, size.h as i32, clip, opacity, x1, y1, x2, y2, color, width);
    }

    // ── 渐变（均有默认实现）──
    fn fill_linear_gradient(&mut self, rect: Rect, color_a: Color, color_b: Color, dir: GradientDirection) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::gradient::fill_linear_gradient(pixels, size.w as i32, size.h as i32, clip, opacity, rect, color_a, color_b, dir);
    }
    fn fill_radial_gradient(&mut self, cx: f32, cy: f32, inner_r: f32, outer_r: f32, inner_color: Color, outer_color: Color) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::gradient::fill_radial_gradient(pixels, size.w as i32, size.h as i32, clip, opacity, cx, cy, inner_r, outer_r, inner_color, outer_color);
    }

    // ── 阴影（均有默认实现）──
    fn draw_box_shadow(&mut self, rect: Rect, blur_radius: f32, offset_x: f32, offset_y: f32, color: Color, corner_radius: Option<Radius>) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::shadow::draw_box_shadow(pixels, size.w as i32, size.h as i32, clip, opacity, rect, blur_radius, offset_x, offset_y, color, corner_radius);
    }
    fn draw_box_shadow_ambient(&mut self, rect: Rect, blur_radius: f32, offset_x: f32, offset_y: f32, color: Color, corner_radius: Option<Radius>) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::shadow::draw_box_shadow_ambient(pixels, size.w as i32, size.h as i32, clip, opacity, rect, blur_radius, offset_x, offset_y, color, corner_radius);
    }

    // ── 图像/字形混合（均有默认实现）──
    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::image::blit_image(pixels, size.w as i32, size.h as i32, clip, opacity, src, src_w, src_rect, dst_rect);
    }
    fn blit_glyph(&mut self, x: i32, y: i32, coverage: &[u8], width: usize, height: usize, color: Color) {
        let (size, clip, opacity) = (self.surface_size(), self.current_clip(), self.opacity());
        let pixels = self.pixels_mut();
        crate::rasterizer::glyph::blit_glyph(pixels, size.w as i32, size.h as i32, clip, opacity, x, y, coverage, width, height, color);
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
    fn width(&self) -> i32 { self.surface_size().w as i32 }
    fn height(&self) -> i32 { self.surface_size().h as i32 }
    fn current_clip(&self) -> Rect;
}

// ════════════════════════════════════════════════════════════════════════════
// 字体文本后端
// ════════════════════════════════════════════════════════════════════════════

pub trait TextBackend: std::fmt::Debug + Send + Sync {
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error>;
    fn unload_font(&mut self, handle: &FontHandle);
    fn is_valid(&self, handle: &FontHandle) -> bool;
    fn has_glyph(&self, font: &FontHandle, ch: char) -> bool;
    fn layout_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> TextLayout;
    fn rasterize_glyph(&self, font: &FontHandle, glyph_id: u32, pixel_size: f32) -> GlyphRaster;
    fn horizontal_line_metrics(&self, font: &FontHandle, pixel_size: f32) -> Option<LineMetrics>;

    fn font_data(&self, _font: &FontHandle) -> Option<Vec<u8>> { None }
    fn set_fallback_fonts(&mut self, _fallback_handles: &[FontHandle]) {}
    fn clear_cache(&mut self) {}
    fn memory_usage(&self) -> usize { 0 }
}

// ════════════════════════════════════════════════════════════════════════════
// 图形引擎
// ════════════════════════════════════════════════════════════════════════════

pub trait GraphicsEngine: 'static {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error>;
    fn shutdown(&mut self);
    fn resize(&mut self, width: i32, height: i32);
    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome;
    fn end_frame(&mut self) -> RenderOutcome;
    fn canvas_2d(&mut self) -> &mut dyn Canvas2D;

    fn dpi(&self) -> f32 { 96.0 }
    fn device_pixel_ratio(&self) -> f32 { 1.0 }
    fn orientation(&self) -> crate::spatial::Orientation { crate::spatial::Orientation::YDown }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> { let _ = (width, height); None }
    fn destroy_offscreen(&mut self, handle: ImageHandle) { let _ = handle; }
    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> { let _ = handle; None }
    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) { let _ = (handle, dst_rect); }

    fn memory_usage(&self) -> usize { 0 }
    fn diagnose_memory(&self) {}
}
