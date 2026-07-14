//! API-neutral non-GL GPU-native raster backend.
//!
//! Hot Canvas2D paths (`fill_rect` / `fill_circle` / `stroke_rect` /
//! `stroke_circle`, axis-aligned `draw_line`, identity solid `blit_glyph`,
//! identity linear/radial gradients, identity fill-rule-aware `fill_path`,
//! cap/join-aware `stroke_path`, and identity box/ambient shadow) draw via
//! [`IGraphicsContext`] operations advertised by [`NativeRasterCaps`]. Every
//! unsupported operation deterministically soft-rasterizes into a CPU buffer
//! and alpha-blits at present.

use std::any::Any;
use std::sync::Arc;

use crate::core::{DamageRegion, Errc, Error, Point, PresentDamageTracker, Rect};
use crate::draw::backend::traits::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::pipeline::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FrameEncoder, FrameRasterOp,
    ReferenceFrame,
};
use crate::draw::primitives::color::Color;
use crate::draw::primitives::path::{FillRule, Path};
use crate::draw::primitives::stroker::StrokeOptions;
use crate::draw::primitives::tessellator;
use crate::draw::primitives::types::{
    BlendMode, GradientDirection, ImageHandle, Radius, Transform,
};
use crate::draw::traits::Canvas2D;
use crate::native::traits::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect, IGraphicsContext, NativeRasterCaps, OffscreenTargetId,
    PresentFrame, PresentMode, PresentTestResult, RasterMode, SoftFallbackTile,
};

#[derive(Clone)]
pub(crate) struct StateSnapshot {
    clip_rect: Rect,
    opacity: f32,
    offset_x: f32,
    offset_y: f32,
    transform: Transform,
    blend_mode: BlendMode,
}

pub(crate) struct PendingNativeRect {
    rect: GpuSolidRect,
    /// Logical scissor AABB (x, y, w, h).
    scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeStroke {
    rect: GpuStrokeRect,
    scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeGlyph {
    glyph: GpuGlyphBlit,
    scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeLinearGrad {
    rect: GpuLinearGradientRect,
    scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeRadialGrad {
    grad: GpuRadialGradient,
    scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeMesh {
    mesh: GpuSolidMesh,
    scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeShadow {
    shadow: GpuBoxShadow,
    scissor: (i32, i32, i32, i32),
}

pub(crate) enum PendingNativeOp {
    SolidRect(PendingNativeRect),
    StrokeRect(PendingNativeStroke),
    Glyph(PendingNativeGlyph),
    LinearGradient(PendingNativeLinearGrad),
    RadialGradient(PendingNativeRadialGrad),
    SolidMesh(PendingNativeMesh),
    BoxShadow(PendingNativeShadow),
}

impl PendingNativeOp {
    fn scissor(&self) -> (i32, i32, i32, i32) {
        match self {
            Self::SolidRect(op) => op.scissor,
            Self::StrokeRect(op) => op.scissor,
            Self::Glyph(op) => op.scissor,
            Self::LinearGradient(op) => op.scissor,
            Self::RadialGradient(op) => op.scissor,
            Self::SolidMesh(op) => op.scissor,
            Self::BoxShadow(op) => op.scissor,
        }
    }

    fn same_kind(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::SolidRect(_), Self::SolidRect(_))
                | (Self::StrokeRect(_), Self::StrokeRect(_))
                | (Self::Glyph(_), Self::Glyph(_))
                | (Self::LinearGradient(_), Self::LinearGradient(_))
                | (Self::RadialGradient(_), Self::RadialGradient(_))
                | (Self::SolidMesh(_), Self::SolidMesh(_))
                | (Self::BoxShadow(_), Self::BoxShadow(_))
        )
    }
}

/// Hybrid Canvas2D: native solid/stroke/glyphs/gradients/paths/shadows + soft fallback.
pub struct NativeGpuCanvas2D {
    native_caps: NativeRasterCaps,
    /// Allocated on first soft-path use (#105) — pure-native frames keep no CPU framebuffer.
    pub(crate) soft_fallback: Option<SharedRasterizer>,
    /// Soft buffer has content that must be composited (until full clear).
    pub(crate) soft_has_content: bool,
    /// Destination-dependent blend cannot be faithfully composed from a
    /// transparent CPU segment over native output.
    soft_uses_destination_blend: bool,
    /// Immediate Canvas2D calls that have no `Result` return channel record
    /// an error here. The frame boundary consumes it before any present.
    deferred_error: Option<Error>,
    pub(crate) pending_native: Vec<PendingNativeOp>,
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
    /// Soft-upload byte count from the last successful soft fallback blit (diagnostics).
    pub(crate) last_soft_upload_bytes: usize,
}

impl NativeGpuCanvas2D {
    pub(crate) fn new(width: i32, height: i32, native_caps: NativeRasterCaps) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        Self {
            native_caps,
            soft_fallback: None,
            soft_has_content: false,
            soft_uses_destination_blend: false,
            deferred_error: None,
            pending_native: Vec::new(),
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
            last_soft_upload_bytes: 0,
        }
    }

    #[cfg(feature = "d3d11")]
    pub(crate) fn pending_mesh_count(&self) -> usize {
        self.pending_native
            .iter()
            .filter(|op| matches!(op, PendingNativeOp::SolidMesh(_)))
            .count()
    }

    pub(crate) fn ensure_soft(&mut self) -> &mut SharedRasterizer {
        let width = self.surface_w;
        let height = self.surface_h;
        self.soft_fallback
            .get_or_insert_with(|| SharedRasterizer::new(PixelSurface::new(width, height)))
    }

    fn resize(&mut self, width: i32, height: i32) {
        let w = width.max(1);
        let h = height.max(1);
        self.surface_w = w;
        self.surface_h = h;
        self.clip_rect = Rect::new(0.0, 0.0, w as f32, h as f32);
        self.clip_stack.clear();
        self.state_stack.clear();
        self.pending_native.clear();
        // Drop soft buffer on resize; recreate lazily at the new size.
        self.soft_fallback = None;
        self.soft_has_content = false;
        self.soft_uses_destination_blend = false;
        self.deferred_error = None;
    }

    pub(crate) fn clear_soft(&mut self) {
        if let Some(soft) = self.soft_fallback.as_mut() {
            soft.surface_mut().clear_all();
        }
        self.soft_has_content = false;
        self.soft_uses_destination_blend = false;
        self.pending_native.clear();
    }

    fn mark_soft(&mut self) {
        self.soft_has_content = true;
    }

    pub(crate) fn take_deferred_error(&mut self) -> Option<Error> {
        self.deferred_error.take()
    }

    fn reject_unsupported(&mut self, operation: &str) {
        if self.deferred_error.is_none() {
            self.deferred_error = Some(Error::new(
                Errc::NotImplemented,
                format!("NativeGpuCanvas2D does not implement {operation}"),
            ));
        }
    }

    fn reject_path_clip(&mut self) {
        self.reject_unsupported("path clip");
    }

    fn clear_soft_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if let Some(soft) = self.soft_fallback.as_mut() {
            soft.surface_mut().clear_rect_raw(x, y, w, h);
        }
    }

    fn sync_fallback_state(&mut self) {
        let transform = self.transform;
        let opacity = self.opacity;
        let blend_mode = self.blend_mode;
        let offset_x = self.offset_x;
        let offset_y = self.offset_y;
        let soft = self.ensure_soft();
        soft.set_transform(transform);
        soft.set_opacity(opacity);
        soft.set_blend_mode(blend_mode);
        soft.set_offset(offset_x, offset_y);
        self.soft_uses_destination_blend |= matches!(blend_mode, BlendMode::Additive);
    }

    fn with_soft_clip<F>(&mut self, f: F)
    where
        F: FnOnce(&mut SharedRasterizer),
    {
        self.sync_fallback_state();
        let clip = self.clip_rect;
        let soft = self.ensure_soft();
        soft.push_clip(clip);
        f(soft);
        soft.pop_clip();
    }

    fn queue_solid_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        // Unsupported capability/state routes to soft fallback before enqueue.
        let identity = self.transform.m == Transform::identity().m;
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.solid_rects || !identity || !native_blend {
            self.with_soft_clip(|soft| soft.fill_rect(rect, color, radius));
            self.mark_soft();
            return;
        }
        let r = match radius {
            Some(rad) => [rad.tl, rad.tr, rad.br, rad.bl],
            None => [0.0; 4],
        };
        let scissor = self.scissor_aabb();
        self.pending_native
            .push(PendingNativeOp::SolidRect(PendingNativeRect {
                rect: GpuSolidRect {
                    x: rect.x + self.offset_x,
                    y: rect.y + self.offset_y,
                    w: rect.w,
                    h: rect.h,
                    rgba: self.rgba(color),
                    radius: r,
                },
                scissor,
            }));
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
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.stroke_rects || !identity || !native_blend {
            self.with_soft_clip(|soft| soft.stroke_rect(rect, color, lw, radius));
            self.mark_soft();
            return;
        }
        let r = match radius {
            Some(rad) => [rad.tl, rad.tr, rad.br, rad.bl],
            None => [0.0; 4],
        };
        let scissor = self.scissor_aabb();
        self.pending_native
            .push(PendingNativeOp::StrokeRect(PendingNativeStroke {
                rect: GpuStrokeRect {
                    x: rect.x + self.offset_x,
                    y: rect.y + self.offset_y,
                    w: rect.w,
                    h: rect.h,
                    rgba: self.rgba(color),
                    radius: r,
                    line_width: lw,
                },
                scissor,
            }));
    }

    fn queue_linear_gradient(&mut self, rect: Rect, ca: Color, cb: Color, dir: GradientDirection) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let identity = self.transform.m == Transform::identity().m;
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.linear_gradients || !identity || !native_blend
        {
            self.with_soft_clip(|soft| soft.fill_linear_gradient(rect, ca, cb, dir));
            self.mark_soft();
            return;
        }
        let dir_u = match dir {
            GradientDirection::Horizontal => 0u32,
            GradientDirection::Vertical => 1,
            GradientDirection::DiagonalTLBR => 2,
            GradientDirection::DiagonalBLTR => 3,
        };
        self.pending_native
            .push(PendingNativeOp::LinearGradient(PendingNativeLinearGrad {
                rect: GpuLinearGradientRect {
                    x: rect.x + self.offset_x,
                    y: rect.y + self.offset_y,
                    w: rect.w,
                    h: rect.h,
                    color_a: self.rgba(ca),
                    color_b: self.rgba(cb),
                    dir: dir_u,
                },
                scissor: self.scissor_aabb(),
            }));
    }

    fn queue_radial_gradient(&mut self, cx: f32, cy: f32, ir: f32, or: f32, ic: Color, oc: Color) {
        if or <= 0.0 {
            return;
        }
        let identity = self.transform.m == Transform::identity().m;
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.radial_gradients || !identity || !native_blend
        {
            self.with_soft_clip(|soft| soft.fill_radial_gradient(cx, cy, ir, or, ic, oc));
            self.mark_soft();
            return;
        }
        self.pending_native
            .push(PendingNativeOp::RadialGradient(PendingNativeRadialGrad {
                grad: GpuRadialGradient {
                    cx: cx + self.offset_x,
                    cy: cy + self.offset_y,
                    inner_r: ir.max(0.0),
                    outer_r: or,
                    color_inner: self.rgba(ic),
                    color_outer: self.rgba(oc),
                },
                scissor: self.scissor_aabb(),
            }));
    }

    /// Queue a CPU-tessellated solid mesh, or soft-fallback on failure.
    fn queue_path_mesh(
        &mut self,
        path: &Path,
        color: Color,
        fill_rule: FillRule,
        stroke: Option<&StrokeOptions>,
    ) {
        let identity = self.transform.m == Transform::identity().m;
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.solid_meshes || !identity || !native_blend {
            self.sync_fallback_state();
            let _soft_clip = self.clip_rect;
            self.ensure_soft().push_clip(_soft_clip);
            if let Some(opts) = stroke {
                self.ensure_soft().stroke_path(path, color, opts);
            } else {
                self.ensure_soft().fill_path(path, color, fill_rule);
            }
            self.ensure_soft().pop_clip();
            self.mark_soft();
            return;
        }
        let (ox, oy) = (self.offset_x, self.offset_y);
        let translated = if ox != 0.0 || oy != 0.0 {
            Some(path.translated(ox, oy))
        } else {
            None
        };
        let path_for_tess = translated.as_ref().unwrap_or(path);
        let verts = if let Some(opts) = stroke {
            tessellator::tessellate_stroke(path_for_tess, opts)
        } else {
            tessellator::tessellate_fill(path_for_tess, fill_rule)
        };
        let Some(verts) = verts else {
            self.sync_fallback_state();
            let _soft_clip = self.clip_rect;
            self.ensure_soft().push_clip(_soft_clip);
            if let Some(opts) = stroke {
                self.ensure_soft().stroke_path(path, color, opts);
            } else {
                self.ensure_soft().fill_path(path, color, fill_rule);
            }
            self.ensure_soft().pop_clip();
            self.mark_soft();
            return;
        };
        if verts.len() < 6 {
            return;
        }
        self.pending_native
            .push(PendingNativeOp::SolidMesh(PendingNativeMesh {
                mesh: GpuSolidMesh {
                    vertices: Arc::<[f32]>::from(verts),
                    rgba: self.rgba(color),
                },
                scissor: self.scissor_aabb(),
            }));
    }

    fn queue_box_shadow(
        &mut self,
        rect: Rect,
        blur: f32,
        ox: f32,
        oy: f32,
        color: Color,
        rad: Option<Radius>,
        ambient: bool,
    ) {
        if rect.w <= 0.0 || rect.h <= 0.0 || color.a == 0 {
            return;
        }
        let identity = self.transform.m == Transform::identity().m;
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.box_shadows || !identity || !native_blend {
            self.sync_fallback_state();
            let _soft_clip = self.clip_rect;
            self.ensure_soft().push_clip(_soft_clip);
            if ambient {
                self.ensure_soft()
                    .draw_box_shadow_ambient(rect, blur, ox, oy, color, rad);
            } else {
                self.ensure_soft()
                    .draw_box_shadow(rect, blur, ox, oy, color, rad);
            }
            self.ensure_soft().pop_clip();
            self.mark_soft();
            return;
        }
        let r = match rad {
            Some(radius) => [radius.tl, radius.tr, radius.br, radius.bl],
            None => [0.0; 4],
        };
        // Apply canvas offset to the source rect (shadow offset is separate).
        let (cox, coy) = (self.offset_x, self.offset_y);
        self.pending_native
            .push(PendingNativeOp::BoxShadow(PendingNativeShadow {
                shadow: GpuBoxShadow {
                    x: rect.x + cox,
                    y: rect.y + coy,
                    w: rect.w,
                    h: rect.h,
                    offset_x: ox,
                    offset_y: oy,
                    blur: blur.max(0.0),
                    rgba: self.rgba(color),
                    radius: r,
                    ambient,
                },
                scissor: self.scissor_aabb(),
            }));
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

    pub(crate) fn submit_native(
        &mut self,
        gpu_ctx: &mut dyn IGraphicsContext,
    ) -> Result<(), Error> {
        let vw = self.surface_w as f32;
        let vh = self.surface_h as f32;
        let mut start = 0;
        while start < self.pending_native.len() {
            let scissor = self.pending_native[start].scissor();
            let mut end = start + 1;
            while end < self.pending_native.len()
                && self.pending_native[start].same_kind(&self.pending_native[end])
                && self.pending_native[end].scissor() == scissor
            {
                end += 1;
            }

            match &self.pending_native[start] {
                PendingNativeOp::SolidRect(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::SolidRect(op) => op.rect,
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_solid_rects(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::StrokeRect(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::StrokeRect(op) => op.rect,
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_stroke_rects(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::Glyph(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::Glyph(op) => op.glyph.clone(),
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_glyphs(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::LinearGradient(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::LinearGradient(op) => op.rect,
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_linear_gradients(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::RadialGradient(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::RadialGradient(op) => op.grad,
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_radial_gradients(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::SolidMesh(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::SolidMesh(op) => op.mesh.clone(),
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_solid_meshes(vw, vh, Some(scissor), &batch)?;
                }
                PendingNativeOp::BoxShadow(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::BoxShadow(op) => op.shadow,
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_box_shadows(vw, vh, Some(scissor), &batch)?;
                }
            }
            start = end;
        }
        Ok(())
    }

    pub(crate) fn submit_soft(&mut self, gpu_ctx: &mut dyn IGraphicsContext) -> Result<(), Error> {
        if !self.soft_has_content {
            return Ok(());
        }
        if self.soft_uses_destination_blend {
            return Err(Error::new(
                Errc::NotImplemented,
                "NativeGpuCanvas2D: CPU fallback cannot emulate destination-dependent Additive blend",
            ));
        }
        let w = self.surface_w;
        let h = self.surface_h;
        let packed = {
            let pixels = self.ensure_soft().surface().pixels();
            pack_visible_soft_fallback_tile(pixels, w, h)
        };
        let Some((pixels, tile)) = packed else {
            return Ok(());
        };
        self.last_soft_upload_bytes = (tile.width as usize)
            .saturating_mul(tile.height as usize)
            .saturating_mul(std::mem::size_of::<u32>());
        gpu_ctx.blit_soft_fallback_tile(&pixels, tile)?;
        Ok(())
    }

    pub(crate) fn commit_presented_frame(&mut self) {
        self.pending_native.clear();
        if self.soft_has_content {
            self.ensure_soft().surface_mut().clear_all();
            self.soft_has_content = false;
            self.soft_uses_destination_blend = false;
        }
    }
}

/// Computes the smallest alpha-visible source tile for one retained CPU
/// segment. Transparent RGB is deliberately ignored: it cannot affect the
/// alpha-blended target and must not force a texture upload.
pub(crate) fn pack_visible_soft_fallback_tile(
    pixels: &[u32],
    width: i32,
    height: i32,
) -> Option<(Vec<u32>, SoftFallbackTile)> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let expected = (width as usize).checked_mul(height as usize)?;
    if pixels.len() < expected {
        return None;
    }
    let mut left = width;
    let mut top = height;
    let mut right = 0;
    let mut bottom = 0;
    let mut visible = false;
    for y in 0..height {
        let row = y as usize * width as usize;
        for x in 0..width {
            if pixels[row + x as usize] & 0xff00_0000 == 0 {
                continue;
            }
            visible = true;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if !visible {
        return None;
    }
    let tile = SoftFallbackTile::at_destination(left, top, right - left, bottom - top);
    let mut packed = Vec::with_capacity(tile.required_pixels()?);
    for y in top..bottom {
        let row = y as usize * width as usize;
        packed.extend_from_slice(&pixels[row + left as usize..row + right as usize]);
    }
    Some((packed, tile))
}

impl Canvas2D for NativeGpuCanvas2D {
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
        self.sync_fallback_state();
        let _soft_clip = self.clip_rect;
        self.ensure_soft().push_clip(_soft_clip);
        self.ensure_soft().fill_ellipse(rect, color);
        self.ensure_soft().pop_clip();
        self.mark_soft();
    }

    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, sa: f32, ea: f32, color: Color) {
        self.sync_fallback_state();
        let _soft_clip = self.clip_rect;
        self.ensure_soft().push_clip(_soft_clip);
        self.ensure_soft().fill_sector(cx, cy, r, sa, ea, color);
        self.ensure_soft().pop_clip();
        self.mark_soft();
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
        let identity = self.transform.m == Transform::identity().m;
        // Axis-aligned lines → solid fill rect (matches CPU fast path).
        if identity && (x1 - x2).abs() < 1e-6 {
            let half = lw * 0.5;
            let rect = Rect::new(x1 - half, y1.min(y2), lw, (y1 - y2).abs());
            self.queue_solid_rect(rect, color, None);
            return;
        }
        if identity && (y1 - y2).abs() < 1e-6 {
            let half = lw * 0.5;
            let rect = Rect::new(x1.min(x2), y1 - half, (x1 - x2).abs(), lw);
            self.queue_solid_rect(rect, color, None);
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
        let identity = self.transform.m == Transform::identity().m;
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        // Soft path for transforms / exotic blend; identity solid text → atlas.
        if self.soft_has_content || !self.native_caps.glyphs || !identity || !native_blend {
            self.sync_fallback_state();
            let _soft_clip = self.clip_rect;
            self.ensure_soft().push_clip(_soft_clip);
            self.ensure_soft()
                .blit_glyph(x, y, coverage.as_ref(), w, h, color);
            self.ensure_soft().pop_clip();
            self.mark_soft();
            return;
        }
        let (ox, oy) = (self.offset_x, self.offset_y);
        let dx = x as f32 + ox;
        let dy = y as f32 + oy;
        self.pending_native
            .push(PendingNativeOp::Glyph(PendingNativeGlyph {
                glyph: GpuGlyphBlit {
                    x: dx,
                    y: dy,
                    w: w as f32,
                    h: h as f32,
                    rgba: self.rgba(color),
                    coverage,
                    cov_w: w as u32,
                    cov_h: h as u32,
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
        let rect = Rect::new(
            rect.x + self.offset_x,
            rect.y + self.offset_y,
            rect.w,
            rect.h,
        );
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

mod backend;

pub use backend::{NativeGpuBackend, NativeGpuDrawSurface};
