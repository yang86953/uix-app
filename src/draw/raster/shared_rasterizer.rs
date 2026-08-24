//! SharedRasterizer — SoftwareRasterizer + PixelSurface 共享 CPU 光栅化（Phase 8）。
//!
//! GPU 未实现路径与 CPU 后端共用，避免 API-specific canvas 内嵌 CPU 后端。

use crate::core::{Error, Rect};

use crate::draw::Canvas2D;
use crate::draw::geometry::color::Color;
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::types::{BlendMode, GradientDirection, Radius, Transform};
use crate::draw::raster::pixel_surface::PixelSurface;
use crate::draw::raster::software_rasterizer::SoftwareRasterizer;

/// 共享 CPU 光栅化 surface。
pub(crate) struct SharedRasterizer {
    renderer: SoftwareRasterizer,
    surface: PixelSurface,
    deferred_error: Option<Error>,
}

impl SharedRasterizer {
    pub(crate) fn new(surface: PixelSurface) -> Self {
        let w = surface.width();
        let h = surface.height();
        Self {
            renderer: SoftwareRasterizer::new(w, h),
            surface,
            deferred_error: None,
        }
    }

    pub(crate) fn take_deferred_error(&mut self) -> Option<Error> {
        self.deferred_error.take()
    }

    pub(crate) fn surface(&self) -> &PixelSurface {
        &self.surface
    }
    pub(crate) fn surface_mut(&mut self) -> &mut PixelSurface {
        &mut self.surface
    }

    /// Replaces only the pixel target while retaining all canvas state. This
    /// lets lazy callers allocate after transforms and clips are configured.
    pub(crate) fn replace_surface_preserving_state(&mut self, surface: PixelSurface) {
        self.surface = surface;
    }

    /// Resets all transient canvas state for a logical extent while retaining
    /// the current pixel allocation. The allocation may be a lazy placeholder.
    pub(crate) fn reset_state_for_extent(&mut self, width: i32, height: i32) {
        self.renderer.reset_for_extent(width, height);
        self.deferred_error = None;
    }

    pub(crate) fn memory_usage(&self) -> usize {
        self.surface.memory_usage()
    }

    pub(crate) fn set_transform(&mut self, t: Transform) {
        self.renderer.set_transform(t);
    }
    pub(crate) fn current_transform(&self) -> Transform {
        self.renderer.transform()
    }

    pub(crate) fn set_opacity(&mut self, opacity: f32) {
        self.renderer.set_opacity(opacity);
    }

    pub(crate) fn set_blend_mode(&mut self, mode: BlendMode) {
        self.renderer.set_blend_mode(mode);
    }

    pub(crate) fn set_offset(&mut self, dx: f32, dy: f32) {
        self.renderer.set_offset(dx, dy);
    }

    pub(crate) fn push_clip(&mut self, rect: Rect) {
        self.renderer.push_clip(rect);
    }

    pub(crate) fn push_clip_surface(&mut self, rect: Rect) {
        self.renderer.push_clip_surface(rect);
    }

    pub(crate) fn map_rect(&self, rect: Rect) -> Rect {
        self.renderer.map_rect(rect)
    }

    // 暴露路径 coverage mask 事实，供 recorder 禁止不带 mask 的 Native/direct lowering。
    pub(crate) fn has_clip_mask(&self) -> bool {
        // 实际状态仍由 SoftwareRasterizer 唯一持有。
        self.renderer.has_clip_mask()
    }

    pub(crate) fn pop_clip(&mut self) {
        self.renderer.pop_clip();
    }

    // 路径裁剪统一委托给 SoftwareRasterizer 的 mask lowering。
    pub(crate) fn try_push_clip_path(&mut self, path: &Path) -> Result<(), Error> {
        self.renderer.try_push_clip_path(path)
    }
}

impl Canvas2D for SharedRasterizer {
    fn current_transform(&self) -> Transform {
        self.renderer.transform()
    }
    fn set_transform(&mut self, transform: Transform) {
        self.renderer.set_transform(transform);
    }
    fn offset(&self) -> (f32, f32) {
        self.renderer.offset()
    }
    fn set_offset(&mut self, dx: f32, dy: f32) {
        self.renderer.set_offset(dx, dy);
    }
    fn translate(&mut self, dx: f32, dy: f32) {
        let (ox, oy) = self.renderer.offset();
        self.renderer.set_offset(ox + dx, oy + dy);
    }

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
        self.renderer
            .fill_sector(pixels, w, h, cx, cy, r, sa, ea, color);
    }
    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer
            .fill_path(pixels, w, h, path, color, fill_rule);
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, lw: f32, radius: Option<Radius>) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer
            .stroke_rect(pixels, w, h, rect, color, lw, radius);
    }
    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, lw: f32) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer
            .stroke_circle(pixels, w, h, cx, cy, r, color, lw);
    }
    fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer.stroke_path(pixels, w, h, path, color, opts);
    }
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        let (w, h) = (self.surface.width(), self.surface.height());
        let pixels = self.surface.pixels_mut();
        self.renderer
            .draw_line(pixels, w, h, x1, y1, x2, y2, color, width);
    }

    fn fill_linear_gradient(&mut self, rect: Rect, ca: Color, cb: Color, dir: GradientDirection) {
        // 共享执行器统一应用 offset、仿射、clip、opacity 与 blend。
        let (width, height) = (self.surface.width(), self.surface.height());
        // 借用像素目标后直接进入状态完整的软件渐变路径。
        let pixels = self.surface.pixels_mut();
        // 线性渐变在局部空间求值并写回设备像素。
        self.renderer
            .fill_linear_gradient(pixels, width, height, rect, ca, cb, dir);
    }
    fn fill_radial_gradient(&mut self, cx: f32, cy: f32, ir: f32, or: f32, ic: Color, oc: Color) {
        // 读取目标尺寸供共享像素入口做边界校验。
        let (width, height) = (self.surface.width(), self.surface.height());
        // 借用目标像素缓冲。
        let pixels = self.surface.pixels_mut();
        // 径向渐变复用同一仿射与混合状态。
        self.renderer
            .fill_radial_gradient(pixels, width, height, cx, cy, ir, or, ic, oc);
    }

    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur: f32,
        ox: f32,
        oy: f32,
        color: Color,
        rad: Option<Radius>,
    ) {
        // 读取目标尺寸供共享阴影入口安全寻址。
        let (width, height) = (self.surface.width(), self.surface.height());
        // 借用像素目标后直接进入状态完整的软件阴影路径。
        let pixels = self.surface.pixels_mut();
        // 定向阴影在局部空间求值，并复用当前 transform、clip、opacity 与 blend。
        self.renderer
            .draw_box_shadow(pixels, width, height, rect, blur, ox, oy, color, rad);
    }
    fn draw_box_shadow_ambient(
        &mut self,
        rect: Rect,
        blur: f32,
        ox: f32,
        oy: f32,
        color: Color,
        rad: Option<Radius>,
    ) {
        // 读取目标尺寸供共享环境阴影入口安全寻址。
        let (width, height) = (self.surface.width(), self.surface.height());
        // 借用像素目标后执行同一状态边界。
        let pixels = self.surface.pixels_mut();
        // 环境阴影只改变 coverage 曲线，不复制仿射或混合实现。
        self.renderer
            .draw_box_shadow_ambient(pixels, width, height, rect, blur, ox, oy, color, rad);
    }

    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        // 读取目标尺寸供共享像素入口做安全边界校验。
        let (width, height) = (self.surface.width(), self.surface.height());
        // 借用目标像素后进入状态完整的软件图片路径。
        let pixels = self.surface.pixels_mut();
        // 图片采样统一保留 offset 后仿射、clip、opacity 与实际 blend。
        self.renderer
            .blit_image(pixels, width, height, src, src_w, src_rect, dst_rect);
    }
    fn blit_glyph(&mut self, x: i32, y: i32, coverage: &[u8], w: usize, h: usize, color: Color) {
        // 读取目标尺寸供共享像素入口做安全边界校验。
        let (width, height) = (self.surface.width(), self.surface.height());
        // 借用目标像素后进入状态完整的软件字形路径。
        let pixels = self.surface.pixels_mut();
        // 字形采样统一保留 offset 后仿射、coverage、clip、opacity 与实际 blend。
        self.renderer
            .blit_glyph(pixels, width, height, x, y, coverage, w, h, color);
    }

    fn save(&mut self) {
        self.renderer.save();
    }
    fn restore(&mut self) {
        self.renderer.restore();
    }
    fn push_clip(&mut self, rect: Rect) {
        self.renderer.push_clip(rect);
    }
    fn pop_clip(&mut self) {
        self.renderer.pop_clip();
    }
    fn set_opacity(&mut self, opacity: f32) {
        self.renderer.set_opacity(opacity);
    }
    fn opacity(&self) -> f32 {
        self.renderer.opacity()
    }
    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.renderer.set_blend_mode(mode);
    }
    fn push_clip_path(&mut self, path: &Path) {
        // Canvas2D 无返回值，因此把 typed failure 延迟到帧边界消费。
        if let Err(error) = self.try_push_clip_path(path) {
            if self.deferred_error.is_none() {
                self.deferred_error = Some(error);
            }
        }
    }

    fn pixels(&self) -> &[u32] {
        self.surface.pixels()
    }

    #[cfg(test)]
    fn pixels_mut(&mut self) -> &mut [u32] {
        self.surface.pixels_mut()
    }
    fn surface_size(&self) -> crate::core::Size {
        self.surface.surface_size()
    }
    fn current_clip(&self) -> Rect {
        self.renderer.clip_rect()
    }

    fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32) {
        let int_dx = dx.round() as i32;
        let int_dy = dy.round() as i32;
        if int_dx == 0 && int_dy == 0 {
            return;
        }
        let src = Rect::new(
            viewport.x + dx.round(),
            viewport.y + dy.round(),
            viewport.w,
            viewport.h,
        );
        self.surface
            .copy_region(src, viewport.x as i32, viewport.y as i32);
    }
}
