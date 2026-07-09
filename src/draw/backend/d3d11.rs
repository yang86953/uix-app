//! D3D11 GPU-native raster backend (`RasterMode::GpuNative` × `PresentMode::Swapchain`).
//!
//! Hot Canvas2D paths (`fill_rect` / `fill_circle` / `stroke_rect` /
//! `stroke_circle`, axis-aligned `draw_line`, and identity solid `blit_glyph`)
//! draw via [`IGraphicsContext`] native geometry / glyph atlas APIs.
//! Unsupported ops soft-raster into a CPU buffer and alpha-blit at present
//! (same hybrid pattern as GL `GpuCanvas2D`).

use std::any::Any;
use std::sync::Arc;

use crate::core::{DamageRegion, Errc, Error, Point, Rect};
use crate::draw::backend::traits::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::primitives::color::Color;
use crate::draw::primitives::path::{FillRule, Path};
use crate::draw::primitives::stroker::StrokeOptions;
use crate::draw::primitives::types::{BlendMode, GradientDirection, Radius, Transform};
use crate::draw::traits::Canvas2D;
use crate::native::traits::present::{
    GraphicsBackend, GpuGlyphBlit, GpuSolidRect, GpuStrokeRect, IGraphicsContext, PresentFrame,
    RasterMode,
};

#[derive(Clone)]
struct StateSnapshot {
    clip_rect: Rect,
    opacity: f32,
    offset_x: f32,
    offset_y: f32,
    transform: Transform,
    blend_mode: BlendMode,
}

struct PendingNativeRect {
    rect: GpuSolidRect,
    /// Logical scissor AABB (x, y, w, h).
    scissor: (i32, i32, i32, i32),
}

struct PendingNativeStroke {
    rect: GpuStrokeRect,
    scissor: (i32, i32, i32, i32),
}

struct PendingNativeGlyph {
    glyph: GpuGlyphBlit,
    scissor: (i32, i32, i32, i32),
}

/// Hybrid Canvas2D: native solid/stroke/glyphs + soft fallback for the rest.
pub struct D3d11Canvas2D {
    soft_fallback: SharedRasterizer,
    /// Soft buffer has content that must be composited (until full clear).
    soft_has_content: bool,
    pending_rects: Vec<PendingNativeRect>,
    pending_strokes: Vec<PendingNativeStroke>,
    pending_glyphs: Vec<PendingNativeGlyph>,
    clip_rect: Rect,
    clip_stack: Vec<Rect>,
    opacity: f32,
    offset_x: f32,
    offset_y: f32,
    transform: Transform,
    blend_mode: BlendMode,
    state_stack: Vec<StateSnapshot>,
    surface_w: i32,
    surface_h: i32,
}

impl D3d11Canvas2D {
    fn new(width: i32, height: i32) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        Self {
            soft_fallback: SharedRasterizer::new(PixelSurface::new(w, h)),
            soft_has_content: false,
            pending_rects: Vec::new(),
            pending_strokes: Vec::new(),
            pending_glyphs: Vec::new(),
            clip_rect: Rect::new(0.0, 0.0, w as f32, h as f32),
            clip_stack: Vec::new(),
            opacity: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            transform: Transform::identity(),
            blend_mode: BlendMode::default(),
            state_stack: Vec::new(),
            surface_w: w,
            surface_h: h,
        }
    }

    fn resize(&mut self, width: i32, height: i32) {
        let w = width.max(1);
        let h = height.max(1);
        self.surface_w = w;
        self.surface_h = h;
        self.clip_rect = Rect::new(0.0, 0.0, w as f32, h as f32);
        self.clip_stack.clear();
        self.state_stack.clear();
        self.pending_rects.clear();
        self.pending_strokes.clear();
        self.pending_glyphs.clear();
        self.soft_fallback = SharedRasterizer::new(PixelSurface::new(w, h));
        self.soft_has_content = false;
    }

    fn clear_soft(&mut self) {
        self.soft_fallback.surface_mut().clear_all();
        self.soft_has_content = false;
        self.pending_rects.clear();
        self.pending_strokes.clear();
        self.pending_glyphs.clear();
    }

    fn mark_soft(&mut self) {
        self.soft_has_content = true;
    }

    fn clear_soft_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.soft_fallback.surface_mut().clear_rect_raw(x, y, w, h);
    }

    fn sync_fallback_state(&mut self) {
        self.soft_fallback.set_transform(self.transform);
        self.soft_fallback.set_opacity(self.opacity);
        self.soft_fallback.set_blend_mode(self.blend_mode);
        self.soft_fallback.set_offset(self.offset_x, self.offset_y);
    }

    fn queue_solid_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        // Non-identity transform → soft path (matches GL limitation for now).
        let identity = self.transform.m == Transform::identity().m;
        if !identity {
            self.sync_fallback_state();
            self.soft_fallback.push_clip(self.clip_rect);
            self.soft_fallback.fill_rect(rect, color, radius);
            self.soft_fallback.pop_clip();
            self.mark_soft();
            return;
        }
        let r = match radius {
            Some(rad) => [rad.tl, rad.tr, rad.br, rad.bl],
            None => [0.0; 4],
        };
        let scissor = self.scissor_aabb();
        self.pending_rects.push(PendingNativeRect {
            rect: GpuSolidRect {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: rect.h,
                rgba: self.rgba(color),
                radius: r,
            },
            scissor,
        });
    }

    fn queue_stroke_rect(
        &mut self,
        rect: Rect,
        color: Color,
        line_width: f32,
        radius: Option<Radius>,
    ) {
        let lw = line_width.max(0.0);
        if rect.w <= 0.0 || rect.h <= 0.0 || lw <= 0.0 {
            return;
        }
        let identity = self.transform.m == Transform::identity().m;
        if !identity {
            self.sync_fallback_state();
            self.soft_fallback.push_clip(self.clip_rect);
            self.soft_fallback.stroke_rect(rect, color, lw, radius);
            self.soft_fallback.pop_clip();
            self.mark_soft();
            return;
        }
        let r = match radius {
            Some(rad) => [rad.tl, rad.tr, rad.br, rad.bl],
            None => [0.0; 4],
        };
        let scissor = self.scissor_aabb();
        self.pending_strokes.push(PendingNativeStroke {
            rect: GpuStrokeRect {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: rect.h,
                rgba: self.rgba(color),
                radius: r,
                line_width: lw,
            },
            scissor,
        });
    }

    fn rgba(&self, color: Color) -> [f32; 4] {
        [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            (color.a as f32 / 255.0) * self.opacity,
        ]
    }

    fn scissor_aabb(&self) -> (i32, i32, i32, i32) {
        let c = self.clip_rect;
        if c.w <= 0.0 || c.h <= 0.0 {
            (0, 0, 0, 0)
        } else {
            (
                c.x.floor() as i32,
                c.y.floor() as i32,
                c.w.ceil() as i32,
                c.h.ceil() as i32,
            )
        }
    }

    fn flush_native(
        &mut self,
        gpu_ctx: &mut dyn IGraphicsContext,
    ) -> Result<(), Error> {
        let pending = std::mem::take(&mut self.pending_rects);
        let pending_strokes = std::mem::take(&mut self.pending_strokes);
        let pending_glyphs = std::mem::take(&mut self.pending_glyphs);
        let vw = self.surface_w as f32;
        let vh = self.surface_h as f32;
        if !pending.is_empty() {
            let mut i = 0;
            while i < pending.len() {
                let scissor = pending[i].scissor;
                let start = i;
                i += 1;
                while i < pending.len() && pending[i].scissor == scissor {
                    i += 1;
                }
                let batch: Vec<GpuSolidRect> = pending[start..i].iter().map(|p| p.rect).collect();
                gpu_ctx.draw_solid_rects(vw, vh, Some(scissor), &batch)?;
            }
        }
        if !pending_strokes.is_empty() {
            let mut i = 0;
            while i < pending_strokes.len() {
                let scissor = pending_strokes[i].scissor;
                let start = i;
                i += 1;
                while i < pending_strokes.len() && pending_strokes[i].scissor == scissor {
                    i += 1;
                }
                let batch: Vec<GpuStrokeRect> =
                    pending_strokes[start..i].iter().map(|p| p.rect).collect();
                gpu_ctx.draw_stroke_rects(vw, vh, Some(scissor), &batch)?;
            }
        }
        if !pending_glyphs.is_empty() {
            let mut i = 0;
            while i < pending_glyphs.len() {
                let scissor = pending_glyphs[i].scissor;
                let start = i;
                i += 1;
                while i < pending_glyphs.len() && pending_glyphs[i].scissor == scissor {
                    i += 1;
                }
                let batch: Vec<GpuGlyphBlit> = pending_glyphs[start..i]
                    .iter()
                    .map(|p| p.glyph.clone())
                    .collect();
                gpu_ctx.draw_glyphs(vw, vh, Some(scissor), &batch)?;
            }
        }
        Ok(())
    }

    fn flush_soft(&mut self, gpu_ctx: &mut dyn IGraphicsContext) -> Result<(), Error> {
        if !self.soft_has_content {
            return Ok(());
        }
        let pixels = self.soft_fallback.surface().pixels();
        gpu_ctx.blit_soft_fallback(pixels, self.surface_w, self.surface_h)
    }
}

impl Canvas2D for D3d11Canvas2D {
    fn offset(&self) -> (f32, f32) {
        (self.offset_x, self.offset_y)
    }

    fn set_offset(&mut self, dx: f32, dy: f32) {
        self.offset_x = dx;
        self.offset_y = dy;
    }

    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let rect = if ox != 0.0 || oy != 0.0 {
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h)
        } else {
            rect
        };
        self.queue_solid_rect(rect, color, radius);
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let rect = Rect::new(cx - r + ox, cy - r + oy, r * 2.0, r * 2.0);
        self.queue_solid_rect(rect, color, Some(Radius::uniform(r)));
    }

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.fill_ellipse(rect, color);
        self.soft_fallback.pop_clip();
        self.mark_soft();
    }

    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, sa: f32, ea: f32, color: Color) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.fill_sector(cx, cy, r, sa, ea, color);
        self.soft_fallback.pop_clip();
        self.mark_soft();
    }

    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.fill_path(path, color, fill_rule);
        self.soft_fallback.pop_clip();
        self.mark_soft();
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, lw: f32, radius: Option<Radius>) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let rect = if ox != 0.0 || oy != 0.0 {
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h)
        } else {
            rect
        };
        self.queue_stroke_rect(rect, color, lw, radius);
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, lw: f32) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let rect = Rect::new(cx - r + ox, cy - r + oy, r * 2.0, r * 2.0);
        self.queue_stroke_rect(rect, color, lw, Some(Radius::uniform(r)));
    }

    fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.stroke_path(path, color, opts);
        self.soft_fallback.pop_clip();
        self.mark_soft();
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, w: f32) {
        let lw = w.max(0.0);
        if lw <= 0.0 {
            return;
        }
        let (ox, oy) = (self.offset_x, self.offset_y);
        let ax1 = x1 + ox;
        let ay1 = y1 + oy;
        let ax2 = x2 + ox;
        let ay2 = y2 + oy;
        let identity = self.transform.m == Transform::identity().m;
        // Axis-aligned lines → solid fill rect (matches CPU fast path).
        if identity && (ax1 - ax2).abs() < 1e-6 {
            let half = lw * 0.5;
            let rect = Rect::new(ax1 - half, ay1.min(ay2), lw, (ay1 - ay2).abs());
            self.queue_solid_rect(rect, color, None);
            return;
        }
        if identity && (ay1 - ay2).abs() < 1e-6 {
            let half = lw * 0.5;
            let rect = Rect::new(ax1.min(ax2), ay1 - half, (ax1 - ax2).abs(), lw);
            self.queue_solid_rect(rect, color, None);
            return;
        }
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.draw_line(x1, y1, x2, y2, color, lw);
        self.soft_fallback.pop_clip();
        self.mark_soft();
    }

    fn fill_linear_gradient(&mut self, rect: Rect, ca: Color, cb: Color, dir: GradientDirection) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback.fill_linear_gradient(rect, ca, cb, dir);
        self.soft_fallback.pop_clip();
        self.mark_soft();
    }

    fn fill_radial_gradient(&mut self, cx: f32, cy: f32, ir: f32, or: f32, ic: Color, oc: Color) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback
            .fill_radial_gradient(cx, cy, ir, or, ic, oc);
        self.soft_fallback.pop_clip();
        self.mark_soft();
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
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback
            .draw_box_shadow(rect, blur, ox, oy, color, rad);
        self.soft_fallback.pop_clip();
        self.mark_soft();
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
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback
            .draw_box_shadow_ambient(rect, blur, ox, oy, color, rad);
        self.soft_fallback.pop_clip();
        self.mark_soft();
    }

    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        self.sync_fallback_state();
        self.soft_fallback.push_clip(self.clip_rect);
        self.soft_fallback
            .blit_image(src, src_w, src_rect, dst_rect);
        self.soft_fallback.pop_clip();
        self.mark_soft();
    }

    fn blit_glyph(&mut self, x: i32, y: i32, coverage: &[u8], w: usize, h: usize, color: Color) {
        if w == 0 || h == 0 || coverage.is_empty() {
            return;
        }
        let identity = self.transform.m == Transform::identity().m;
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        // Soft path for transforms / exotic blend; identity solid text → atlas.
        if !identity || !native_blend {
            self.sync_fallback_state();
            self.soft_fallback.push_clip(self.clip_rect);
            self.soft_fallback.blit_glyph(x, y, coverage, w, h, color);
            self.soft_fallback.pop_clip();
            self.mark_soft();
            return;
        }
        let (ox, oy) = (self.offset_x, self.offset_y);
        let dx = x as f32 + ox;
        let dy = y as f32 + oy;
        let cov = Arc::<[u8]>::from(coverage.to_vec());
        self.pending_glyphs.push(PendingNativeGlyph {
            glyph: GpuGlyphBlit {
                x: dx,
                y: dy,
                w: w as f32,
                h: h as f32,
                rgba: self.rgba(color),
                coverage: cov,
                cov_w: w as u32,
                cov_h: h as u32,
            },
            scissor: self.scissor_aabb(),
        });
    }

    fn push_clip_path(&mut self, _path: &Path) {}

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

    fn pixels_mut(&mut self) -> &mut [u32] {
        self.mark_soft();
        self.soft_fallback.pixels_mut()
    }

    fn surface_size(&self) -> crate::core::Size {
        crate::core::Size::new(self.surface_w as f32, self.surface_h as f32)
    }

    fn current_clip(&self) -> Rect {
        self.clip_rect
    }
}

/// DrawSurface for D3D11 GpuNative path.
pub struct D3d11DrawSurface {
    canvas: D3d11Canvas2D,
    width: i32,
    height: i32,
    /// Full clear pending (ClearRenderTargetView).
    needs_gpu_clear: bool,
    /// Partial clear rects (replace-blend quads).
    pending_clear_rects: Vec<GpuSolidRect>,
}

impl DrawSurface for D3d11DrawSurface {
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
        // GPU backends do not support scroll memmove.
    }

    fn canvas(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }
}

/// D3D11 `RenderBackend` — native solid/stroke rects + soft fallback + swapchain present.
pub struct D3d11Backend {
    pub gpu_ctx: Box<dyn IGraphicsContext>,
    width: i32,
    height: i32,
    surface: D3d11DrawSurface,
}

impl D3d11Backend {
    pub fn new(mut gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let caps = gpu_ctx.caps();
        if caps.raster != RasterMode::GpuNative || caps.backend != GraphicsBackend::D3d11 {
            let backend = caps.backend;
            let raster = caps.raster;
            gpu_ctx.shutdown();
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d11Backend requires D3D11 GpuNative context, got {backend} raster={raster}"
                ),
            ));
        }
        Ok(Self {
            gpu_ctx,
            width: 0,
            height: 0,
            surface: D3d11DrawSurface {
                canvas: D3d11Canvas2D::new(1, 1),
                width: 1,
                height: 1,
                needs_gpu_clear: true,
                pending_clear_rects: Vec::new(),
            },
        })
    }
}

impl RenderBackend for D3d11Backend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::gpu()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let logical_w = width.max(1);
        let logical_h = height.max(1);
        self.gpu_ctx.resize(logical_w, logical_h);
        self.width = logical_w;
        self.height = logical_h;
        self.surface.width = logical_w;
        self.surface.height = logical_h;
        self.surface.canvas.resize(logical_w, logical_h);
        self.surface.needs_gpu_clear = true;
        self.surface.pending_clear_rects.clear();
        Ok(())
    }

    fn shutdown(&mut self) {
        self.gpu_ctx.shutdown();
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.surface
    }

    fn present(&mut self, damage: &DamageRegion) -> Result<(), Error> {
        self.gpu_ctx.make_current();

        if self.surface.needs_gpu_clear {
            if let Err(err) = self.gpu_ctx.clear_render_target(0.0, 0.0, 0.0, 0.0) {
                crate::core::log::error_fn(format!(
                    "D3d11Backend clear_render_target failed: {}",
                    err.short_what()
                ));
            }
            self.surface.needs_gpu_clear = false;
            self.surface.pending_clear_rects.clear();
        } else if !self.surface.pending_clear_rects.is_empty() {
            let clears = std::mem::take(&mut self.surface.pending_clear_rects);
            if let Err(err) = self.gpu_ctx.clear_rects(
                self.surface.width as f32,
                self.surface.height as f32,
                &clears,
            ) {
                crate::core::log::error_fn(format!(
                    "D3d11Backend clear_rects failed: {}",
                    err.short_what()
                ));
            }
        }

        if let Err(err) = self.surface.canvas.flush_native(self.gpu_ctx.as_mut()) {
            crate::core::log::error_fn(format!(
                "D3d11Backend flush_native failed: {}",
                err.short_what()
            ));
        }
        if let Err(err) = self.surface.canvas.flush_soft(self.gpu_ctx.as_mut()) {
            crate::core::log::error_fn(format!(
                "D3d11Backend flush_soft failed: {}",
                err.short_what()
            ));
        }

        let frame = PresentFrame::Swapchain {
            damage: damage.to_present_damage(),
        };
        if let Err(err) = self.gpu_ctx.present(&frame) {
            crate::core::log::error_fn(format!(
                "D3d11Backend present failed: {}",
                err.short_what()
            ));
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

unsafe impl Send for D3d11Backend {}
unsafe impl Sync for D3d11Backend {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::present::{GraphicsContextCaps, PresentDamage, PresentMode};
    use std::cell::Cell;
    use std::rc::Rc;

    struct FakeD3d11Context {
        clear_calls: Rc<Cell<usize>>,
        clear_rect_calls: Rc<Cell<usize>>,
        draw_calls: Rc<Cell<usize>>,
        stroke_calls: Rc<Cell<usize>>,
        glyph_calls: Rc<Cell<usize>>,
        blit_calls: Rc<Cell<usize>>,
        upload_calls: Rc<Cell<usize>>,
        present_calls: Rc<Cell<usize>>,
        last_draw_count: Rc<Cell<usize>>,
        last_stroke_count: Rc<Cell<usize>>,
        last_glyph_count: Rc<Cell<usize>>,
        width: i32,
        height: i32,
    }

    impl IGraphicsContext for FakeD3d11Context {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, false, 1.0)
        }

        fn initialize(
            &mut self,
            _native_window: *mut std::ffi::c_void,
            _width: i32,
            _height: i32,
        ) -> crate::core::Result<()> {
            Ok(())
        }

        fn resize(&mut self, width: i32, height: i32) {
            self.width = width.max(1);
            self.height = height.max(1);
        }

        fn make_current(&mut self) {}

        fn swap_buffers(&mut self, _damage: PresentDamage) {
            self.present_calls.set(self.present_calls.get() + 1);
        }

        fn shutdown(&mut self) {}

        fn read_pixels(&mut self, _x: i32, _y: i32, _w: i32, _h: i32) -> Vec<u32> {
            Vec::new()
        }

        fn width(&self) -> i32 {
            self.width
        }

        fn height(&self) -> i32 {
            self.height
        }

        fn clear_render_target(
            &mut self,
            _r: f32,
            _g: f32,
            _b: f32,
            _a: f32,
        ) -> crate::core::Result<()> {
            self.clear_calls.set(self.clear_calls.get() + 1);
            Ok(())
        }

        fn supports_native_geometry(&self) -> bool {
            true
        }

        fn draw_solid_rects(
            &mut self,
            _viewport_w: f32,
            _viewport_h: f32,
            _scissor: Option<(i32, i32, i32, i32)>,
            rects: &[GpuSolidRect],
        ) -> crate::core::Result<()> {
            self.draw_calls.set(self.draw_calls.get() + 1);
            self.last_draw_count.set(rects.len());
            Ok(())
        }

        fn draw_stroke_rects(
            &mut self,
            _viewport_w: f32,
            _viewport_h: f32,
            _scissor: Option<(i32, i32, i32, i32)>,
            rects: &[GpuStrokeRect],
        ) -> crate::core::Result<()> {
            self.stroke_calls.set(self.stroke_calls.get() + 1);
            self.last_stroke_count.set(rects.len());
            Ok(())
        }

        fn supports_native_glyphs(&self) -> bool {
            true
        }

        fn draw_glyphs(
            &mut self,
            _viewport_w: f32,
            _viewport_h: f32,
            _scissor: Option<(i32, i32, i32, i32)>,
            glyphs: &[GpuGlyphBlit],
        ) -> crate::core::Result<()> {
            self.glyph_calls.set(self.glyph_calls.get() + 1);
            self.last_glyph_count.set(glyphs.len());
            Ok(())
        }

        fn blit_soft_fallback(
            &mut self,
            _pixels: &[u32],
            _width: i32,
            _height: i32,
        ) -> crate::core::Result<()> {
            self.blit_calls.set(self.blit_calls.get() + 1);
            Ok(())
        }

        fn clear_rects(
            &mut self,
            _viewport_w: f32,
            _viewport_h: f32,
            rects: &[GpuSolidRect],
        ) -> crate::core::Result<()> {
            self.clear_rect_calls
                .set(self.clear_rect_calls.get() + rects.len());
            Ok(())
        }

        fn upload_surface_pixels(
            &mut self,
            pixels: &[u32],
            width: i32,
            height: i32,
        ) -> crate::core::Result<()> {
            assert_eq!(pixels.len(), (width * height) as usize);
            self.upload_calls.set(self.upload_calls.get() + 1);
            Ok(())
        }
    }

    fn fake_ctx(
        clear_calls: &Rc<Cell<usize>>,
        clear_rect_calls: &Rc<Cell<usize>>,
        draw_calls: &Rc<Cell<usize>>,
        stroke_calls: &Rc<Cell<usize>>,
        glyph_calls: &Rc<Cell<usize>>,
        blit_calls: &Rc<Cell<usize>>,
        upload_calls: &Rc<Cell<usize>>,
        present_calls: &Rc<Cell<usize>>,
        last_draw_count: &Rc<Cell<usize>>,
        last_stroke_count: &Rc<Cell<usize>>,
        last_glyph_count: &Rc<Cell<usize>>,
    ) -> FakeD3d11Context {
        FakeD3d11Context {
            clear_calls: Rc::clone(clear_calls),
            clear_rect_calls: Rc::clone(clear_rect_calls),
            draw_calls: Rc::clone(draw_calls),
            stroke_calls: Rc::clone(stroke_calls),
            glyph_calls: Rc::clone(glyph_calls),
            blit_calls: Rc::clone(blit_calls),
            upload_calls: Rc::clone(upload_calls),
            present_calls: Rc::clone(present_calls),
            last_draw_count: Rc::clone(last_draw_count),
            last_stroke_count: Rc::clone(last_stroke_count),
            last_glyph_count: Rc::clone(last_glyph_count),
            width: 1,
            height: 1,
        }
    }

    #[test]
    fn d3d11_backend_rejects_non_d3d11_context() {
        struct GlCaps;
        impl IGraphicsContext for GlCaps {
            fn caps(&self) -> GraphicsContextCaps {
                GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::OpenGlEs, false, 1.0)
            }
            fn initialize(
                &mut self,
                _: *mut std::ffi::c_void,
                _: i32,
                _: i32,
            ) -> crate::core::Result<()> {
                Ok(())
            }
            fn resize(&mut self, _: i32, _: i32) {}
            fn make_current(&mut self) {}
            fn swap_buffers(&mut self, _: PresentDamage) {}
            fn shutdown(&mut self) {}
            fn read_pixels(&mut self, _: i32, _: i32, _: i32, _: i32) -> Vec<u32> {
                Vec::new()
            }
            fn width(&self) -> i32 {
                1
            }
            fn height(&self) -> i32 {
                1
            }
        }
        let err = match D3d11Backend::new(Box::new(GlCaps)) {
            Ok(_) => panic!("expected reject"),
            Err(err) => err,
        };
        assert_eq!(err.code(), Errc::InvalidArgument);
    }

    #[test]
    fn d3d11_backend_present_uses_native_rects_not_full_upload() {
        let clear_calls = Rc::new(Cell::new(0usize));
        let clear_rect_calls = Rc::new(Cell::new(0usize));
        let draw_calls = Rc::new(Cell::new(0usize));
        let stroke_calls = Rc::new(Cell::new(0usize));
        let glyph_calls = Rc::new(Cell::new(0usize));
        let blit_calls = Rc::new(Cell::new(0usize));
        let upload_calls = Rc::new(Cell::new(0usize));
        let present_calls = Rc::new(Cell::new(0usize));
        let last_draw_count = Rc::new(Cell::new(0usize));
        let last_stroke_count = Rc::new(Cell::new(0usize));
        let last_glyph_count = Rc::new(Cell::new(0usize));
        let mut backend = D3d11Backend::new(Box::new(fake_ctx(
            &clear_calls,
            &clear_rect_calls,
            &draw_calls,
            &stroke_calls,
            &glyph_calls,
            &blit_calls,
            &upload_calls,
            &present_calls,
            &last_draw_count,
            &last_stroke_count,
            &last_glyph_count,
        )))
        .expect("backend");
        backend.resize(64, 48).expect("resize");
        {
            let canvas = backend.surface().canvas();
            canvas.fill_rect(
                Rect::new(4.0, 8.0, 20.0, 12.0),
                Color::from_rgb(255, 0, 0),
                None,
            );
            canvas.fill_rect(
                Rect::new(10.0, 10.0, 8.0, 8.0),
                Color::from_rgb(0, 255, 0),
                Some(Radius::uniform(2.0)),
            );
        }
        backend.present(&DamageRegion::full()).expect("present");
        assert_eq!(clear_calls.get(), 1);
        assert_eq!(draw_calls.get(), 1);
        assert_eq!(last_draw_count.get(), 2);
        assert_eq!(stroke_calls.get(), 0);
        assert_eq!(glyph_calls.get(), 0);
        assert_eq!(blit_calls.get(), 0);
        assert_eq!(upload_calls.get(), 0);
        assert_eq!(present_calls.get(), 1);
        let _ = PresentMode::Swapchain;
        let _ = (clear_rect_calls.get(), last_stroke_count.get(), last_glyph_count.get());
    }

    #[test]
    fn d3d11_backend_native_strokes_without_soft_blit() {
        let clear_calls = Rc::new(Cell::new(0usize));
        let clear_rect_calls = Rc::new(Cell::new(0usize));
        let draw_calls = Rc::new(Cell::new(0usize));
        let stroke_calls = Rc::new(Cell::new(0usize));
        let glyph_calls = Rc::new(Cell::new(0usize));
        let blit_calls = Rc::new(Cell::new(0usize));
        let upload_calls = Rc::new(Cell::new(0usize));
        let present_calls = Rc::new(Cell::new(0usize));
        let last_draw_count = Rc::new(Cell::new(0usize));
        let last_stroke_count = Rc::new(Cell::new(0usize));
        let last_glyph_count = Rc::new(Cell::new(0usize));
        let mut backend = D3d11Backend::new(Box::new(fake_ctx(
            &clear_calls,
            &clear_rect_calls,
            &draw_calls,
            &stroke_calls,
            &glyph_calls,
            &blit_calls,
            &upload_calls,
            &present_calls,
            &last_draw_count,
            &last_stroke_count,
            &last_glyph_count,
        )))
        .expect("backend");
        backend.resize(96, 64).expect("resize");
        {
            let canvas = backend.surface().canvas();
            canvas.stroke_rect(
                Rect::new(8.0, 8.0, 40.0, 24.0),
                Color::from_rgb(0, 128, 255),
                2.0,
                Some(Radius::uniform(4.0)),
            );
            canvas.stroke_circle(70.0, 32.0, 16.0, Color::from_rgb(255, 200, 0), 3.0);
            canvas.draw_line(0.0, 50.0, 90.0, 50.0, Color::black(), 2.0);
        }
        backend.present(&DamageRegion::full()).expect("present");
        assert_eq!(clear_calls.get(), 1);
        assert_eq!(stroke_calls.get(), 1);
        assert_eq!(last_stroke_count.get(), 2);
        // Axis-aligned draw_line → solid fill batch.
        assert_eq!(draw_calls.get(), 1);
        assert_eq!(last_draw_count.get(), 1);
        assert_eq!(glyph_calls.get(), 0);
        assert_eq!(blit_calls.get(), 0);
        assert_eq!(upload_calls.get(), 0);
        assert_eq!(present_calls.get(), 1);
        let _ = (clear_rect_calls.get(), last_glyph_count.get());
    }

    #[test]
    fn d3d11_backend_native_glyphs_without_soft_blit() {
        let clear_calls = Rc::new(Cell::new(0usize));
        let clear_rect_calls = Rc::new(Cell::new(0usize));
        let draw_calls = Rc::new(Cell::new(0usize));
        let stroke_calls = Rc::new(Cell::new(0usize));
        let glyph_calls = Rc::new(Cell::new(0usize));
        let blit_calls = Rc::new(Cell::new(0usize));
        let upload_calls = Rc::new(Cell::new(0usize));
        let present_calls = Rc::new(Cell::new(0usize));
        let last_draw_count = Rc::new(Cell::new(0usize));
        let last_stroke_count = Rc::new(Cell::new(0usize));
        let last_glyph_count = Rc::new(Cell::new(0usize));
        let mut backend = D3d11Backend::new(Box::new(fake_ctx(
            &clear_calls,
            &clear_rect_calls,
            &draw_calls,
            &stroke_calls,
            &glyph_calls,
            &blit_calls,
            &upload_calls,
            &present_calls,
            &last_draw_count,
            &last_stroke_count,
            &last_glyph_count,
        )))
        .expect("backend");
        backend.resize(64, 48).expect("resize");
        {
            let canvas = backend.surface().canvas();
            canvas.fill_rect(Rect::new(0.0, 0.0, 8.0, 8.0), Color::white(), None);
            let cov = [255u8; 4 * 6];
            canvas.blit_glyph(10, 12, &cov, 4, 6, Color::from_rgb(0, 0, 0));
            canvas.blit_glyph(20, 12, &cov, 4, 6, Color::from_rgb(32, 32, 32));
        }
        backend.present(&DamageRegion::full()).expect("present");
        assert_eq!(clear_calls.get(), 1);
        assert_eq!(draw_calls.get(), 1);
        assert_eq!(glyph_calls.get(), 1);
        assert_eq!(last_glyph_count.get(), 2);
        assert_eq!(blit_calls.get(), 0);
        assert_eq!(upload_calls.get(), 0);
        assert_eq!(present_calls.get(), 1);
        let _ = (
            clear_rect_calls.get(),
            stroke_calls.get(),
            last_draw_count.get(),
            last_stroke_count.get(),
        );
    }

    #[test]
    fn d3d11_backend_soft_ops_blit_without_full_upload() {
        let clear_calls = Rc::new(Cell::new(0usize));
        let clear_rect_calls = Rc::new(Cell::new(0usize));
        let draw_calls = Rc::new(Cell::new(0usize));
        let stroke_calls = Rc::new(Cell::new(0usize));
        let glyph_calls = Rc::new(Cell::new(0usize));
        let blit_calls = Rc::new(Cell::new(0usize));
        let upload_calls = Rc::new(Cell::new(0usize));
        let present_calls = Rc::new(Cell::new(0usize));
        let last_draw_count = Rc::new(Cell::new(0usize));
        let last_stroke_count = Rc::new(Cell::new(0usize));
        let last_glyph_count = Rc::new(Cell::new(0usize));
        let mut backend = D3d11Backend::new(Box::new(fake_ctx(
            &clear_calls,
            &clear_rect_calls,
            &draw_calls,
            &stroke_calls,
            &glyph_calls,
            &blit_calls,
            &upload_calls,
            &present_calls,
            &last_draw_count,
            &last_stroke_count,
            &last_glyph_count,
        )))
        .expect("backend");
        backend.resize(32, 32).expect("resize");
        {
            let canvas = backend.surface().canvas();
            canvas.fill_rect(Rect::new(0.0, 0.0, 8.0, 8.0), Color::white(), None);
            // Diagonal line stays soft.
            canvas.draw_line(0.0, 0.0, 10.0, 10.0, Color::black(), 1.0);
        }
        backend.present(&DamageRegion::full()).expect("present");
        assert_eq!(draw_calls.get(), 1);
        assert_eq!(stroke_calls.get(), 0);
        assert_eq!(glyph_calls.get(), 0);
        assert_eq!(blit_calls.get(), 1);
        assert_eq!(upload_calls.get(), 0);
        assert_eq!(present_calls.get(), 1);
        let _ = (
            clear_calls.get(),
            clear_rect_calls.get(),
            last_draw_count.get(),
            last_stroke_count.get(),
            last_glyph_count.get(),
        );
    }
}
