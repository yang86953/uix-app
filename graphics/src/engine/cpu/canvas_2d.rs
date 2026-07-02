//! CPU 2D 绘制上下文——实现 Canvas2D trait。
//!
//! CpuCanvas2D = RasterRenderer(通用渲染逻辑) + PixelSurface(像素输出目标)。
//! 所有渲染逻辑在 RasterRenderer 中，这里只做委托。

use uix_platform::Rect;

use crate::color::Color;
use crate::engine::cpu::pixel_surface::PixelSurface;
use crate::engine::cpu::raster_renderer::RasterRenderer;
use crate::path::{FillRule, Path};
use crate::stroker::StrokeOptions;
use crate::traits::Canvas2D;
use crate::types::{BlendMode, GradientDirection, Radius, Transform};

/// CPU 2D 绘制实现。
pub struct CpuCanvas2D {
    /// 通用渲染器（状态+逻辑）。
    renderer: RasterRenderer,
    /// 底层像素表面。
    surface: PixelSurface,
}

impl CpuCanvas2D {
    pub fn new(surface: PixelSurface) -> Self {
        let w = surface.width();
        let h = surface.height();
        Self {
            renderer: RasterRenderer::new(w, h),
            surface,
        }
    }

    pub fn surface(&self) -> &PixelSurface { &self.surface }
    pub fn surface_mut(&mut self) -> &mut PixelSurface { &mut self.surface }

    pub fn set_transform(&mut self, t: Transform) { self.renderer.set_transform(t); }
    pub fn reset_transform(&mut self) { self.renderer.set_transform(Transform::identity()); }
}

impl Canvas2D for CpuCanvas2D {
    fn offset(&self) -> (f32, f32) { self.renderer.offset() }
    fn set_offset(&mut self, dx: f32, dy: f32) { self.renderer.set_offset(dx, dy); }
    fn translate(&mut self, dx: f32, dy: f32) {
        let (ox, oy) = self.renderer.offset();
        self.renderer.set_offset(ox + dx, oy + dy);
    }

    // ── 矢量填充 ──
    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer.fill_rect(pixels, w, h, rect, color, radius);
    }
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer.fill_circle(pixels, w, h, cx, cy, r, color);
    }
    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer.fill_ellipse(pixels, w, h, rect, color);
    }
    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, sa: f32, ea: f32, color: Color) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer.fill_sector(pixels, w, h, cx, cy, r, sa, ea, color);
    }
    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer.fill_path(pixels, w, h, path, color, fill_rule);
    }

    // ── 矢量描边 ──
    fn stroke_rect(&mut self, rect: Rect, color: Color, lw: f32, radius: Option<Radius>) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer.stroke_rect(pixels, w, h, rect, color, lw, radius);
    }
    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, lw: f32) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer.stroke_circle(pixels, w, h, cx, cy, r, color, lw);
    }
    fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer.stroke_path(pixels, w, h, path, color, opts);
    }
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer.draw_line(pixels, w, h, x1, y1, x2, y2, color, width);
    }

    // ── 渐变 ──
    fn fill_linear_gradient(&mut self, rect: Rect, ca: Color, cb: Color, dir: GradientDirection) {
        let (ox, oy) = self.renderer.offset();
        let rect = if ox != 0.0 || oy != 0.0 { Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h) } else { rect };
        let (size, clip, opacity) = (self.surface.surface_size(), self.renderer.clip_rect(), self.renderer.opacity());
        let pixels = self.surface.pixels_mut();
        crate::rasterizer::gradient::fill_linear_gradient(pixels, size.w as i32, size.h as i32, clip, opacity, rect, ca, cb, dir);
    }
    fn fill_radial_gradient(&mut self, cx: f32, cy: f32, ir: f32, or: f32, ic: Color, oc: Color) {
        let (ox, oy) = self.renderer.offset();
        let (cx, cy) = if ox != 0.0 || oy != 0.0 { (cx + ox, cy + oy) } else { (cx, cy) };
        let (size, clip, opacity) = (self.surface.surface_size(), self.renderer.clip_rect(), self.renderer.opacity());
        let pixels = self.surface.pixels_mut();
        crate::rasterizer::gradient::fill_radial_gradient(pixels, size.w as i32, size.h as i32, clip, opacity, cx, cy, ir, or, ic, oc);
    }

    // ── 阴影 ──
    fn draw_box_shadow(&mut self, rect: Rect, blur: f32, ox: f32, oy: f32, color: Color, rad: Option<Radius>) {
        let (oxs, oys) = self.renderer.offset();
        let rect = if oxs != 0.0 || oys != 0.0 { Rect::new(rect.x + oxs, rect.y + oys, rect.w, rect.h) } else { rect };
        let (size, clip, opacity) = (self.surface.surface_size(), self.renderer.clip_rect(), self.renderer.opacity());
        let pixels = self.surface.pixels_mut();
        crate::rasterizer::shadow::draw_box_shadow(pixels, size.w as i32, size.h as i32, clip, opacity, rect, blur, ox, oy, color, rad);
    }
    fn draw_box_shadow_ambient(&mut self, rect: Rect, blur: f32, ox: f32, oy: f32, color: Color, rad: Option<Radius>) {
        let (oxs, oys) = self.renderer.offset();
        let rect = if oxs != 0.0 || oys != 0.0 { Rect::new(rect.x + oxs, rect.y + oys, rect.w, rect.h) } else { rect };
        let (size, clip, opacity) = (self.surface.surface_size(), self.renderer.clip_rect(), self.renderer.opacity());
        let pixels = self.surface.pixels_mut();
        crate::rasterizer::shadow::draw_box_shadow_ambient(pixels, size.w as i32, size.h as i32, clip, opacity, rect, blur, ox, oy, color, rad);
    }

    // ── 图像/字形 ──
    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        let (ox, oy) = self.renderer.offset();
        let dst = if ox != 0.0 || oy != 0.0 { Rect::new(dst_rect.x + ox, dst_rect.y + oy, dst_rect.w, dst_rect.h) } else { dst_rect };
        let (size, clip, opacity) = (self.surface.surface_size(), self.renderer.clip_rect(), self.renderer.opacity());
        let pixels = self.surface.pixels_mut();
        crate::rasterizer::image::blit_image(pixels, size.w as i32, size.h as i32, clip, opacity, src, src_w, src_rect, dst);
    }
    fn blit_glyph(&mut self, x: i32, y: i32, coverage: &[u8], w: usize, h: usize, color: Color) {
        let (ox, oy) = self.renderer.offset();
        let (x, y) = ((x as f32 + ox) as i32, (y as f32 + oy) as i32);
        let (size, clip, opacity) = (self.surface.surface_size(), self.renderer.clip_rect(), self.renderer.opacity());
        let pixels = self.surface.pixels_mut();
        crate::rasterizer::glyph::blit_glyph(pixels, size.w as i32, size.h as i32, clip, opacity, x, y, coverage, w, h, color);
    }

    // ── 渲染状态栈 ──
    fn save(&mut self) { self.renderer.save(); }
    fn restore(&mut self) { self.renderer.restore(); }
    fn push_clip(&mut self, rect: Rect) { self.renderer.push_clip(rect); }
    fn pop_clip(&mut self) { self.renderer.pop_clip(); }
    fn set_opacity(&mut self, opacity: f32) { self.renderer.set_opacity(opacity); }
    fn opacity(&self) -> f32 { self.renderer.opacity() }
    fn set_blend_mode(&mut self, mode: BlendMode) { /* raster_renderer 暂不处理 blend_mode */ let _ = mode; }
    fn push_clip_path(&mut self, _path: &Path) {}

    // ── 像素访问 ──
    fn pixels_mut(&mut self) -> &mut [u32] { self.surface.pixels_mut() }
    fn surface_size(&self) -> uix_platform::Size { self.surface.surface_size() }
    fn current_clip(&self) -> Rect { self.renderer.clip_rect() }

    // ── 像素移动 ──
    fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32) {
        use crate::traits::RenderingBackend;
        let int_dx = dx.round() as i32;
        let int_dy = dy.round() as i32;
        if int_dx == 0 && int_dy == 0 { return; }
        // dx/dy 是滚动偏移增量（scroll 增加量）。
        // 当 scroll 增加 dy>0 时，内容向上移动，像素应从 viewport.y+dy 复制到 viewport.y。
        // 源矩形 = viewport + delta（从旧位置读取像素）。
        let src = Rect::new(viewport.x + dx, viewport.y + dy, viewport.w, viewport.h);
        RenderingBackend::copy_region(&mut self.surface, src, viewport.x as i32, viewport.y as i32);
    }
}
