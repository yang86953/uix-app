//! [`NativeGpuCanvas2D`] 的 `Canvas2D` 实现 — gpu 子模块。
//!
//! 热路径直接入队 native 命令；unsupported 操作确定性软光栅化并在 present
//! 时 alpha-blit（GPU-only 模式 typed 失败）。

use std::sync::Arc;

use crate::core::{Point, Rect};
use crate::draw::geometry::color::Color;
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::types::{BlendMode, GradientDirection, ImageHandle, Radius, Transform};
use crate::draw::painting::{
    FrameCommand, FrameEncoder, FrameEncoderError, FrameGlyphBlit, FrameRect, FrameRasterOp,
    FrameStrokeRect, ReferenceFrame,
};
use crate::draw::raster::pixel_surface::PixelSurface;
use crate::draw::raster::shared_rasterizer::SharedRasterizer;
use crate::draw::Canvas2D;
use crate::native::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSector,
    GpuSolidMesh, GpuSolidRect, GpuStrokeRect, IGraphicsContext, NativeRasterCaps,
    OffscreenTargetId, PresentFrame, PresentMode, PresentTestResult, RasterMode, SoftFallbackTile,
};

use super::canvas::NativeGpuCanvas2D;
use super::geometry::{
    glyph_device_corners, outline_uses_area_r8, quad_aabb, uniform_transform_scale,
};
use super::pending::StateSnapshot;
use super::pending::{
    DirectImageBlit, PendingNativeGlyph, PendingNativeImage, PendingNativeLinearGrad,
    PendingNativeMesh, PendingNativeOp, PendingNativeRadialGrad, PendingNativeRect,
    PendingNativeSector, PendingNativeShadow, PendingNativeStroke,
};

impl Canvas2D for NativeGpuCanvas2D {
    fn current_transform(&self) -> Transform {
        self.transform
    }

    fn set_transform(&mut self, transform: Transform) {
        self.transform = transform;
    }

    fn offset(&self) -> (f32, f32) {
        (self.offset_x, self.offset_y)
    }

    fn set_offset(&mut self, dx: f32, dy: f32) {
        self.offset_x = dx;
        self.offset_y = dy;
    }

    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        self.queue_solid_rect(rect, color, radius);
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let rect = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        self.queue_solid_rect(rect, color, Some(Radius::uniform(r)));
    }

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        if self.gpu_only {
            self.queue_ellipse_mesh(rect, color);
            return;
        }
        self.sync_fallback_state();
        let _soft_clip = self.clip_rect;
        self.ensure_soft().push_clip(_soft_clip);
        self.ensure_soft().fill_ellipse(rect, color);
        self.ensure_soft().pop_clip();
        self.mark_soft();
    }

    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, sa: f32, ea: f32, color: Color) {
        self.queue_sector_mesh(cx, cy, r, sa, ea, color);
    }

    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        self.queue_path_mesh(path, color, fill_rule, None);
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, lw: f32, radius: Option<Radius>) {
        self.queue_stroke_rect(rect, color, lw, radius);
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, lw: f32) {
        let rect = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        self.queue_stroke_rect(rect, color, lw, Some(Radius::uniform(r)));
    }

    fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        self.queue_path_mesh(path, color, FillRule::NonZero, Some(opts));
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, w: f32) {
        let lw = w.max(0.0);
        if lw <= 0.0 {
            return;
        }
        let p1 = self
            .transform
            .transform_point(Point::new(x1 + self.offset_x, y1 + self.offset_y));
        let p2 = self
            .transform
            .transform_point(Point::new(x2 + self.offset_x, y2 + self.offset_y));
        let stroke_scale = uniform_transform_scale(self.transform).unwrap_or(1.0);
        let stroke_w = lw * stroke_scale;
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        let axis_aligned_vertical = (p1.x - p2.x).abs() < 1e-6;
        let axis_aligned_horizontal = (p1.y - p2.y).abs() < 1e-6;
        if (axis_aligned_vertical || axis_aligned_horizontal)
            && !self.soft_has_content
            && self.native_caps.solid_rects
            && native_blend
        {
            let half = stroke_w * 0.5;
            let rect = if axis_aligned_vertical {
                Rect::new(p1.x - half, p1.y.min(p2.y), stroke_w, (p1.y - p2.y).abs())
            } else {
                Rect::new(p1.x.min(p2.x), p1.y - half, (p1.x - p2.x).abs(), stroke_w)
            };
            if rect.w > 0.0 && rect.h > 0.0 {
                self.pending_native
                    .push(PendingNativeOp::SolidRect(PendingNativeRect {
                        rect: GpuSolidRect {
                            x: rect.x,
                            y: rect.y,
                            w: rect.w,
                            h: rect.h,
                            rgba: self.solid_rgba(color),
                            radius: [0.0; 4],
                        },
                        scissor: self.scissor_aabb(),
                    }));
                return;
            }
        }
        // 对角线：以设备坐标线段为中轴构造描边四边形网格。
        if !self.soft_has_content && self.native_caps.solid_meshes && native_blend {
            let dx = p2.x - p1.x;
            let dy = p2.y - p1.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 1e-6 && stroke_w > 0.0 {
                let nx = -dy / len * stroke_w * 0.5;
                let ny = dx / len * stroke_w * 0.5;
                let (x0, y0) = (p1.x + nx, p1.y + ny);
                let (x1, y1) = (p1.x - nx, p1.y - ny);
                let (x2, y2) = (p2.x - nx, p2.y - ny);
                let (x3, y3) = (p2.x + nx, p2.y + ny);
                self.pending_native
                    .push(PendingNativeOp::SolidMesh(PendingNativeMesh {
                        mesh: GpuSolidMesh {
                            vertices: Arc::<[f32]>::from(vec![
                                x0, y0, x1, y1, x2, y2, x0, y0, x2, y2, x3, y3,
                            ]),
                            rgba: self.solid_rgba(color),
                        },
                        scissor: self.scissor_aabb(),
                    }));
                return;
            }
        }
        if self.gpu_only {
            self.reject_unsupported("diagonal line GPU primitive");
            return;
        }
        self.sync_fallback_state();
        let _soft_clip = self.clip_rect;
        self.ensure_soft().push_clip(_soft_clip);
        self.ensure_soft().draw_line(x1, y1, x2, y2, color, lw);
        self.ensure_soft().pop_clip();
        self.mark_soft();
    }

    fn fill_linear_gradient(&mut self, rect: Rect, ca: Color, cb: Color, dir: GradientDirection) {
        self.queue_linear_gradient(rect, ca, cb, dir);
    }

    fn fill_radial_gradient(&mut self, cx: f32, cy: f32, ir: f32, or: f32, ic: Color, oc: Color) {
        self.queue_radial_gradient(cx, cy, ir, or, ic, oc);
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
        self.queue_box_shadow(rect, blur, ox, oy, color, rad, false);
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
        self.queue_box_shadow(rect, blur, ox, oy, color, rad, true);
    }

    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        if !self.opacity.is_finite() || self.opacity <= 0.0 {
            return;
        }
        if !self.soft_has_content {
            match self.try_queue_direct_image_blit(src, src_w, src_rect, dst_rect) {
                DirectImageBlit::Ready(blit) => {
                    self.pending_native
                        .push(PendingNativeOp::ImageBlit(PendingNativeImage {
                            blit,
                            scissor: self.scissor_aabb(),
                        }));
                    return;
                }
                DirectImageBlit::Culled => return,
                DirectImageBlit::Unsupported => {}
            }
        }
        if self.gpu_only {
            self.reject_unsupported("image GPU texture blit");
            return;
        }
        self.sync_fallback_state();
        let _soft_clip = self.clip_rect;
        self.ensure_soft().push_clip(_soft_clip);
        self.ensure_soft()
            .blit_image(src, src_w, src_rect, dst_rect);
        self.ensure_soft().pop_clip();
        self.mark_soft();
    }

    fn blit_glyph(&mut self, x: i32, y: i32, coverage: &[u8], w: usize, h: usize, color: Color) {
        if w == 0 || h == 0 || coverage.is_empty() {
            return;
        }
        self.blit_glyph_shared(x, y, std::sync::Arc::<[u8]>::from(coverage), w, h, color);
    }

    fn blit_glyph_shared(
        &mut self,
        x: i32,
        y: i32,
        coverage: std::sync::Arc<[u8]>,
        w: usize,
        h: usize,
        color: Color,
    ) {
        if w == 0 || h == 0 || coverage.is_empty() {
            return;
        }
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        // Soft path for exotic blend；任意仿射仍可走 atlas 纹理四边形。
        if self.soft_has_content || !self.native_caps.glyphs || !native_blend {
            if self.gpu_only {
                self.reject_unsupported("destination-dependent glyph blend");
                return;
            }
            self.sync_fallback_state();
            let _soft_clip = self.clip_rect;
            self.ensure_soft().push_clip(_soft_clip);
            self.ensure_soft()
                .blit_glyph(x, y, coverage.as_ref(), w, h, color);
            self.ensure_soft().pop_clip();
            self.mark_soft();
            return;
        }
        let local = Rect::new(x as f32, y as f32, w as f32, h as f32);
        let corners = glyph_device_corners(local, self.transform, self.offset_x, self.offset_y);
        let (min_x, min_y, max_x, max_y) = quad_aabb(corners);
        let device_w = max_x - min_x;
        let device_h = max_y - min_y;
        if !device_w.is_finite() || !device_h.is_finite() || device_w <= 0.0 || device_h <= 0.0 {
            return;
        }
        self.pending_native
            .push(PendingNativeOp::Glyph(PendingNativeGlyph {
                glyph: GpuGlyphBlit {
                    x: min_x,
                    y: min_y,
                    w: device_w,
                    h: device_h,
                    corners,
                    rgba: self.rgba(color),
                    coverage,
                    cov_w: w as u32,
                    cov_h: h as u32,
                    outline_mesh: None,
                },
                scissor: self.scissor_aabb(),
            }));
    }

    fn blit_glyph_outline(
        &mut self,
        x: i32,
        y: i32,
        mesh: std::sync::Arc<[f32]>,
        w: usize,
        h: usize,
        color: Color,
    ) {
        self.blit_glyph_outline_shared(x, y, mesh, None, w, h, color);
    }

    fn blit_glyph_outline_shared(
        &mut self,
        x: i32,
        y: i32,
        mesh: std::sync::Arc<[f32]>,
        area_coverage: Option<std::sync::Arc<[u8]>>,
        w: usize,
        h: usize,
        color: Color,
    ) {
        if w == 0
            || h == 0
            || !crate::draw::resources::font::glyph_outline::is_outline_edges(mesh.as_ref())
        {
            return;
        }
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.glyphs || !native_blend {
            if self.gpu_only {
                self.reject_unsupported("destination-dependent glyph blend");
                return;
            }
            // soft：优先复用字体光栅器给出的面积 coverage。
            self.sync_fallback_state();
            let _soft_clip = self.clip_rect;
            self.ensure_soft().push_clip(_soft_clip);
            self.ensure_soft()
                .blit_glyph_outline_shared(x, y, mesh, area_coverage, w, h, color);
            self.ensure_soft().pop_clip();
            self.mark_soft();
            return;
        }
        let local = Rect::new(x as f32, y as f32, w as f32, h as f32);
        let corners = glyph_device_corners(local, self.transform, self.offset_x, self.offset_y);
        let (min_x, min_y, max_x, max_y) = quad_aabb(corners);
        let device_w = max_x - min_x;
        let device_h = max_y - min_y;
        if !device_w.is_finite() || !device_h.is_finite() || device_w <= 0.0 || device_h <= 0.0 {
            return;
        }
        // 物理 1:1：面积 coverage → R8 atlas；缩放、仿射及高 DPR 走 MSDF。
        if outline_uses_area_r8(
            self.transform,
            self.device_pixel_ratio,
            device_w,
            device_h,
            w,
            h,
        ) {
            let expected = w.saturating_mul(h);
            if let Some(coverage) = area_coverage.filter(|c| c.len() >= expected) {
                self.pending_native
                    .push(PendingNativeOp::Glyph(PendingNativeGlyph {
                        glyph: GpuGlyphBlit {
                            x: min_x,
                            y: min_y,
                            w: device_w,
                            h: device_h,
                            corners,
                            rgba: self.rgba(color),
                            coverage,
                            cov_w: w as u32,
                            cov_h: h as u32,
                            outline_mesh: None,
                        },
                        scissor: self.scissor_aabb(),
                    }));
                return;
            }
        }
        self.pending_native
            .push(PendingNativeOp::Glyph(PendingNativeGlyph {
                glyph: GpuGlyphBlit {
                    x: min_x,
                    y: min_y,
                    w: device_w,
                    h: device_h,
                    corners,
                    rgba: self.rgba(color),
                    coverage: std::sync::Arc::from([]),
                    cov_w: w as u32,
                    cov_h: h as u32,
                    outline_mesh: Some(mesh),
                },
                scissor: self.scissor_aabb(),
            }));
    }

    fn push_clip_path(&mut self, _path: &Path) {
        self.reject_path_clip();
    }

    fn save(&mut self) {
        self.state_stack.push(StateSnapshot {
            clip_rect: self.clip_rect,
            opacity: self.opacity,
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            transform: self.transform,
            blend_mode: self.blend_mode,
        });
    }

    fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            self.clip_rect = state.clip_rect;
            self.opacity = state.opacity;
            self.offset_x = state.offset_x;
            self.offset_y = state.offset_y;
            self.transform = state.transform;
            self.blend_mode = state.blend_mode;
        }
    }

    fn push_clip(&mut self, rect: Rect) {
        let rect = self.transform.transform_rect(Rect::new(
            rect.x + self.offset_x,
            rect.y + self.offset_y,
            rect.w,
            rect.h,
        ));
        self.clip_stack.push(self.clip_rect);
        if let Some(intersection) = self.clip_rect.intersect(&rect) {
            self.clip_rect = intersection;
        } else {
            self.clip_rect = Rect::zero();
        }
    }

    fn pop_clip(&mut self) {
        if let Some(prev) = self.clip_stack.pop() {
            self.clip_rect = prev;
        }
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    fn opacity(&self) -> f32 {
        self.opacity
    }

    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend_mode = mode;
    }

    fn pixels(&self) -> &[u32] {
        self.soft_fallback
            .as_ref()
            .map_or(&[], |soft| soft.surface().pixels())
    }

    #[cfg(test)]
    fn pixels_mut(&mut self) -> &mut [u32] {
        if self.gpu_only {
            self.reject_unsupported("direct CPU pixel access in GPU-only mode");
            return &mut self.rejected_pixels;
        }
        self.mark_soft();
        self.soft_uses_destination_blend |= matches!(self.blend_mode, BlendMode::Additive);
        self.ensure_soft().pixels_mut()
    }

    fn surface_size(&self) -> crate::core::Size {
        crate::core::Size::new(self.surface_w as f32, self.surface_h as f32)
    }

    fn current_clip(&self) -> Rect {
        self.clip_rect
    }

    fn scroll_region(&mut self, _viewport: Rect, _dx: f32, _dy: f32) {
        self.reject_unsupported("scroll-region copy");
    }
}
