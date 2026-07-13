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

use crate::core::{DamageRegion, Errc, Error, Point, Rect};
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
    PresentFrame, PresentMode, RasterMode, SoftFallbackTile,
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

/// DrawSurface for an API-neutral `GpuNative × Swapchain` path.
pub struct NativeGpuDrawSurface {
    pub(crate) canvas: NativeGpuCanvas2D,
    native_caps: NativeRasterCaps,
    pub(crate) width: i32,
    pub(crate) height: i32,
    /// Full clear pending (ClearRenderTargetView).
    pub(crate) needs_gpu_clear: bool,
    /// Partial clear rects (replace-blend quads).
    pub(crate) pending_clear_rects: Vec<GpuSolidRect>,
}

impl DrawSurface for NativeGpuDrawSurface {
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
        if !self.native_caps.clear_rects {
            self.pending_clear_rects.clear();
            self.needs_gpu_clear = true;
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
        self.canvas.reject_unsupported("scroll-region copy");
    }

    fn canvas(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    fn take_deferred_error(&mut self) -> Option<Error> {
        self.canvas.take_deferred_error()
    }
}

/// Capability-driven non-GL `RenderBackend` with deterministic soft fallback.
pub struct NativeGpuBackend {
    pub(crate) gpu_ctx: Box<dyn IGraphicsContext>,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) surface: NativeGpuDrawSurface,
    pub(crate) offscreens: Vec<Option<NativeGpuOffscreen>>,
    pub(crate) free_offscreen_ids: Vec<u32>,
    next_offscreen_id: u32,
    /// Picture paint currently targeting this offscreen handle id.
    pub(crate) active_offscreen: Option<u32>,
    /// A draw-time operation without a `Result` return path (for example an
    /// immediate Picture blit) failed.  The failure is reported from the sole
    /// final present boundary, so callers keep the frame dirty instead of
    /// treating an incomplete command stream as committed.
    frame_failure: Option<Error>,
    pub(crate) shutdown: bool,
}

pub(crate) struct NativeGpuOffscreen {
    target: OffscreenTargetId,
    pub(crate) canvas: NativeGpuCanvas2D,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

/// Canvas coordinates are logical pixels. Native contexts report their
/// drawable extent through `width`/`height`, so derive the matching logical
/// extent from their single DPR source before allocating draw-side state.
fn logical_extent_from_context(gpu_ctx: &dyn IGraphicsContext) -> (i32, i32) {
    let dpr = gpu_ctx.device_pixel_ratio();
    let dpr = if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    };
    let logical = |drawable: i32| ((drawable.max(1) as f32 / dpr).round() as i32).max(1);
    (logical(gpu_ctx.width()), logical(gpu_ctx.height()))
}

impl NativeGpuBackend {
    pub(crate) fn new(mut gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let caps = gpu_ctx.caps();
        let native_caps = gpu_ctx.native_raster_caps();
        if caps.raster != RasterMode::GpuNative
            || caps.present != PresentMode::Swapchain
            || !native_caps.has_hybrid_baseline()
        {
            let backend = caps.backend;
            let raster = caps.raster;
            let present = caps.present;
            gpu_ctx.try_shutdown()?;
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "NativeGpuBackend requires GpuNative × Swapchain plus clear/soft-blit baseline, got {backend} raster={raster} present={present} native={native_caps:?}"
                ),
            ));
        }
        let (logical_w, logical_h) = logical_extent_from_context(gpu_ctx.as_ref());
        Ok(Self {
            gpu_ctx,
            width: logical_w,
            height: logical_h,
            shutdown: false,
            offscreens: Vec::new(),
            free_offscreen_ids: Vec::new(),
            next_offscreen_id: 0,
            active_offscreen: None,
            frame_failure: None,
            surface: NativeGpuDrawSurface {
                canvas: NativeGpuCanvas2D::new(logical_w, logical_h, native_caps),
                native_caps,
                width: logical_w,
                height: logical_h,
                needs_gpu_clear: true,
                pending_clear_rects: Vec::new(),
            },
        })
    }

    /// Flushes the current ordered segment without presenting, then reads the
    /// native drawable. This is a crate-local diagnostic/test boundary; it
    /// deliberately uses the same command ordering as a final present.
    #[cfg(all(test, any(feature = "opengles", feature = "d3d11", feature = "d3d12")))]
    pub(crate) fn try_readback(&mut self) -> Result<Vec<u32>, Error> {
        if self.active_offscreen.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "cannot read the swapchain while an offscreen target is active",
            ));
        }
        self.flush_main_segment_before_ordered_boundary()?;
        let width = self.gpu_ctx.width().max(1);
        let height = self.gpu_ctx.height().max(1);
        self.gpu_ctx.read_pixels(0, 0, width, height)
    }

    #[cfg(all(test, any(feature = "opengles", feature = "d3d11", feature = "d3d12")))]
    pub(crate) fn last_soft_upload_bytes(&self) -> usize {
        self.surface.canvas.last_soft_upload_bytes
    }

    fn destroy_all_offscreens(&mut self) -> Result<(), Error> {
        self.active_offscreen = None;
        self.gpu_ctx.bind_swapchain_target()?;
        let handles = self
            .offscreens
            .iter()
            .enumerate()
            .filter_map(|(id, target)| target.as_ref().map(|_| ImageHandle(id as u32)))
            .collect::<Vec<_>>();
        for handle in handles {
            self.try_destroy_offscreen(handle)?;
        }
        self.offscreens.clear();
        self.free_offscreen_ids.clear();
        self.next_offscreen_id = 0;
        Ok(())
    }

    fn remember_frame_failure(&mut self, error: Error) {
        if self.frame_failure.is_none() {
            self.frame_failure = Some(error);
        }
        // Commands before an immediate boundary may already have reached the
        // target.  The next retained-dirty retry must start from a known full
        // clear rather than alpha-blending on that partial target.
        self.surface.needs_gpu_clear = true;
    }

    fn adopt_factory_drawable_extent(&mut self) -> (i32, i32) {
        let (logical_w, logical_h) = logical_extent_from_context(self.gpu_ctx.as_ref());
        self.width = logical_w;
        self.height = logical_h;
        self.surface.width = logical_w;
        self.surface.height = logical_h;
        self.surface.canvas.resize(logical_w, logical_h);
        self.surface.needs_gpu_clear = true;
        self.surface.pending_clear_rects.clear();
        (logical_w, logical_h)
    }

    /// Submit all commands that precede an immediate ordered operation such
    /// as a Picture/offscreen blit.  This is deliberately *not* a present:
    /// it only establishes the exact painter-order boundary inside the one
    /// frame and leaves final swap/present to [`RenderBackend::present`].
    fn flush_main_segment_before_ordered_boundary(&mut self) -> Result<(), Error> {
        self.gpu_ctx.make_current()?;

        if self.surface.needs_gpu_clear {
            self.gpu_ctx.clear_render_target(0.0, 0.0, 0.0, 0.0)?;
            self.surface.needs_gpu_clear = false;
            self.surface.pending_clear_rects.clear();
        } else if !self.surface.pending_clear_rects.is_empty() {
            if let Err(err) = self.gpu_ctx.clear_rects(
                self.surface.width as f32,
                self.surface.height as f32,
                &self.surface.pending_clear_rects,
            ) {
                self.surface.needs_gpu_clear = true;
                return Err(err);
            }
            self.surface.pending_clear_rects.clear();
        }

        if let Err(err) = self.surface.canvas.submit_native(self.gpu_ctx.as_mut()) {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }
        if let Err(err) = self.surface.canvas.submit_soft(self.gpu_ctx.as_mut()) {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }
        self.surface.canvas.commit_presented_frame();
        Ok(())
    }

    /// Executes the API-neutral stream at each recorded command boundary.
    /// `Native` maps to the GPU-native DTO, while CPU segments and Picture
    /// blits produce isolated transparent sources and are alpha-uploaded at
    /// their original painter-order position. This deliberately does not
    /// upload a completed `render_reference()` frame.
    fn execute_frame_encoder(&mut self, encoder: &FrameEncoder) -> Result<(), Error> {
        let mut target_initialized = false;
        for command in encoder.commands() {
            match command {
                FrameCommand::Clear { color } => {
                    self.clear_frame_encoder_target(*color)?;
                    target_initialized = true;
                }
                FrameCommand::Native { operation } => {
                    self.ensure_frame_encoder_target(&mut target_initialized)?;
                    self.execute_native_frame_operation(
                        encoder.width(),
                        encoder.height(),
                        operation,
                    )?;
                }
                FrameCommand::CpuSegment { image, src, dst } => {
                    self.ensure_frame_encoder_target(&mut target_initialized)?;
                    let source = encoder.cpu_segment_reference(image, *src, *dst);
                    self.alpha_blit_frame_encoder_source(&source)?;
                }
                FrameCommand::PictureBlit { image, src, dst } => {
                    self.ensure_frame_encoder_target(&mut target_initialized)?;
                    let source = encoder.picture_blit_reference(image, *src, *dst);
                    self.alpha_blit_frame_encoder_source(&source)?;
                }
            }
        }
        if !target_initialized {
            self.clear_frame_encoder_target(Color::transparent())?;
        }
        Ok(())
    }

    fn ensure_frame_encoder_target(&mut self, target_initialized: &mut bool) -> Result<(), Error> {
        if !*target_initialized {
            self.clear_frame_encoder_target(Color::transparent())?;
            *target_initialized = true;
        }
        Ok(())
    }

    fn clear_frame_encoder_target(&mut self, color: Color) -> Result<(), Error> {
        self.gpu_ctx.clear_render_target(
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        )
    }

    fn execute_native_frame_operation(
        &mut self,
        target_width: i32,
        target_height: i32,
        operation: &FrameRasterOp,
    ) -> Result<(), Error> {
        match operation {
            FrameRasterOp::FillRect { rect, color } => {
                if rect.width <= 0 || rect.height <= 0 {
                    return Ok(());
                }
                self.gpu_ctx.draw_solid_rects(
                    target_width as f32,
                    target_height as f32,
                    None,
                    &[GpuSolidRect {
                        x: rect.x as f32,
                        y: rect.y as f32,
                        w: rect.width as f32,
                        h: rect.height as f32,
                        rgba: [
                            color.r as f32 / 255.0,
                            color.g as f32 / 255.0,
                            color.b as f32 / 255.0,
                            color.a as f32 / 255.0,
                        ],
                        radius: [0.0; 4],
                    }],
                )
            }
            FrameRasterOp::FillRectAdditive { .. } | FrameRasterOp::ScrollCopy { .. } => {
                self.execute_destination_dependent_frame_op(target_width, target_height, operation)
            }
        }
    }

    /// Destination-dependent IR ops cannot be drawn with SrcOver GPU quads.
    /// When the context supports readback + full upload, apply the reference
    /// semantics on CPU pixels and replace the RT — pixel-correct, not an
    /// alpha-over approximation. Otherwise return typed NotImplemented.
    fn execute_destination_dependent_frame_op(
        &mut self,
        target_width: i32,
        target_height: i32,
        operation: &FrameRasterOp,
    ) -> Result<(), Error> {
        let expected = (target_width as usize).saturating_mul(target_height as usize);
        let mut pixels = self
            .gpu_ctx
            .read_pixels(0, 0, target_width, target_height)
            .map_err(|error| {
                if error.code() == Errc::NotImplemented {
                    Error::new(
                        Errc::NotImplemented,
                        "NativeGpuBackend: destination-dependent FrameRasterOp requires readback",
                    )
                    .with_source(error)
                } else {
                    error
                }
            })?;
        if pixels.len() != expected {
            return Err(Error::new(
                Errc::InvalidState,
                format!(
                    "NativeGpuBackend: destination-dependent FrameRasterOp readback extent mismatch (got {}, expected {expected})",
                    pixels.len()
                ),
            ));
        }
        crate::draw::pipeline::frame_encoder::apply_frame_raster_op(
            target_width,
            target_height,
            &mut pixels,
            operation,
        );
        self.gpu_ctx
            .upload_surface_pixels(&pixels, target_width, target_height)
            .map_err(|error| {
                if error.code() == Errc::NotImplemented {
                    Error::new(
                        Errc::NotImplemented,
                        "NativeGpuBackend: destination-dependent FrameRasterOp requires replace upload",
                    )
                    .with_source(error)
                } else {
                    error
                }
            })
    }

    fn alpha_blit_frame_encoder_source(&mut self, source: &ReferenceFrame) -> Result<(), Error> {
        if let Some((pixels, tile)) =
            pack_visible_soft_fallback_tile(source.pixels(), source.width(), source.height())
        {
            self.gpu_ctx.blit_soft_fallback_tile(&pixels, tile)?;
        }
        Ok(())
    }
}

impl RenderBackend for NativeGpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        let partial = self.gpu_ctx.caps().partial_present && self.surface.native_caps.clear_rects;
        let mut caps = if partial {
            BackendCapabilities::gpu()
        } else {
            BackendCapabilities::gpu_full_redraw()
        };
        caps.offscreen = self.surface.native_caps.offscreen_targets;
        caps
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let logical_w = width.max(1);
        let logical_h = height.max(1);
        self.gpu_ctx.resize(logical_w, logical_h)?;
        // D3D11/D3D12 等会按 HWND GetClientRect 校正缓冲尺寸；canvas/布局必须跟
        // 实际 RT 一致，否则清出更大黑底而 UI 仍画旧几何 → 窗口黑边。
        self.adopt_factory_drawable_extent();
        Ok(())
    }

    fn initialize_prepared(&mut self, _width: i32, _height: i32) -> Result<(i32, i32), Error> {
        // `IGraphicsContext::initialize` already ran in the factory against
        // the real surface. Startup only synchronizes draw-owned state to the
        // factory-reported drawable; it must not recreate the swapchain.
        Ok(self.adopt_factory_drawable_extent())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        if self.shutdown {
            return Ok(());
        }
        self.destroy_all_offscreens()?;
        self.gpu_ctx.try_shutdown()?;
        self.shutdown = true;
        Ok(())
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.surface
    }

    fn make_current(&mut self) -> Result<(), Error> {
        self.gpu_ctx.make_current()
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.gpu_ctx.device_pixel_ratio()
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        if !self.surface.native_caps.offscreen_targets || width <= 0 || height <= 0 {
            return None;
        }
        let target = self
            .gpu_ctx
            .create_offscreen_target(width, height)
            .inspect_err(|err| {
                crate::core::log::warn_fn(format!(
                    "NativeGpuBackend: create_offscreen_target failed: {}",
                    err.short_what()
                ));
            })
            .ok()?;
        let id = if let Some(id) = self.free_offscreen_ids.pop() {
            id
        } else {
            let id = self.next_offscreen_id;
            self.next_offscreen_id = self.next_offscreen_id.saturating_add(1);
            id
        };
        let idx = id as usize;
        while self.offscreens.len() <= idx {
            self.offscreens.push(None);
        }
        self.offscreens[idx] = Some(NativeGpuOffscreen {
            target,
            canvas: NativeGpuCanvas2D::new(width, height, self.surface.native_caps),
            width,
            height,
        });
        Some(ImageHandle(id))
    }

    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        let idx = handle.0 as usize;
        let Some(off) = self.offscreens.get(idx).and_then(Option::as_ref) else {
            return Ok(());
        };
        if self.active_offscreen == Some(handle.0) {
            self.gpu_ctx.bind_swapchain_target()?;
        }
        self.gpu_ctx.try_destroy_offscreen_target(off.target)?;
        self.offscreens[idx] = None;
        if self.active_offscreen == Some(handle.0) {
            self.active_offscreen = None;
        }
        self.free_offscreen_ids.push(handle.0);
        Ok(())
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        if let Err(error) = self.try_destroy_offscreen(handle) {
            self.remember_frame_failure(error);
        }
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let idx = handle.0 as usize;
        self.offscreens
            .get_mut(idx)?
            .as_mut()
            .map(|o| &mut o.canvas as &mut dyn Canvas2D)
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        if self.active_offscreen != Some(handle.0) {
            return Err(Error::new(
                Errc::InvalidState,
                "FrameEncoder Picture execution requires its bound offscreen target",
            ));
        }
        let target = self
            .offscreens
            .get(handle.0 as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "Picture offscreen target disappeared before FrameEncoder execution",
                )
            })?;
        if (target.width, target.height) != (encoder.width(), encoder.height()) {
            return Err(Error::new(
                Errc::InvalidState,
                format!(
                    "FrameEncoder {}x{} does not match Picture target {}x{}",
                    encoder.width(),
                    encoder.height(),
                    target.width,
                    target.height
                ),
            ));
        }
        let target = target.target;

        self.gpu_ctx.bind_offscreen_target(target)?;
        self.execute_frame_encoder(encoder)?;
        Ok(EncodedPictureExecution::Executed)
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        if self.active_offscreen.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "main FrameEncoder execution cannot run while a Picture target is bound",
            ));
        }
        if (self.width, self.height) != (encoder.width(), encoder.height()) {
            return Err(Error::new(
                Errc::InvalidState,
                format!(
                    "FrameEncoder {}x{} does not match main native target {}x{}",
                    encoder.width(),
                    encoder.height(),
                    self.width,
                    self.height
                ),
            ));
        }

        let execute = (|| {
            self.gpu_ctx.make_current()?;
            self.gpu_ctx.bind_swapchain_target()?;
            self.execute_frame_encoder(encoder)
        })();
        if let Err(error) = execute {
            self.surface.needs_gpu_clear = true;
            return Err(error);
        }

        // `begin_frame` may have prepared a clear or retained Canvas2D state.
        // The FrameEncoder has replaced the target, so final `present` must
        // not submit a second clear/draw sequence over it.
        self.surface.needs_gpu_clear = false;
        self.surface.pending_clear_rects.clear();
        self.surface.canvas.commit_presented_frame();
        Ok(EncodedFrameExecution::Executed)
    }

    fn begin_offscreen_paint(&mut self, handle: &ImageHandle) -> bool {
        match self.try_begin_offscreen_paint(handle) {
            Ok(()) => true,
            Err(error) => {
                self.remember_frame_failure(error);
                false
            }
        }
    }

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen target does not exist",
            ));
        };
        let target = off.target;
        self.gpu_ctx.bind_offscreen_target(target)?;
        if let Err(error) = self.gpu_ctx.clear_render_target(0.0, 0.0, 0.0, 0.0) {
            return match self.gpu_ctx.bind_swapchain_target() {
                Ok(()) => Err(error),
                Err(restore_error) => Err(restore_error.with_source(error)),
            };
        }
        self.active_offscreen = Some(handle.0);
        Ok(())
    }

    fn flush_offscreen_paint(&mut self, handle: &ImageHandle) {
        if let Err(error) = self.try_flush_offscreen_paint(handle) {
            // The legacy void entry remains for old callers.  The production
            // compositor uses `try_*` and therefore returns this failure
            // before a final present can be reported as success.
            self.remember_frame_failure(error);
        }
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        let idx = handle.0 as usize;
        let off = self
            .offscreens
            .get_mut(idx)
            .and_then(Option::as_mut)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "offscreen target disappeared before flush",
                )
            })?;
        if let Some(error) = off.canvas.take_deferred_error() {
            return Err(error);
        }
        let target = off.target;
        self.gpu_ctx.bind_offscreen_target(target)?;
        off.canvas.submit_native(self.gpu_ctx.as_mut())?;
        off.canvas.submit_soft(self.gpu_ctx.as_mut())?;
        off.canvas.commit_presented_frame();
        Ok(())
    }

    fn end_offscreen_paint(&mut self) {
        self.active_offscreen = None;
        if let Err(error) = self.try_end_offscreen_paint() {
            self.remember_frame_failure(error);
        }
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.active_offscreen = None;
        self.gpu_ctx.bind_swapchain_target()
    }

    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        let Some(Some(off)) = self.offscreens.get(handle.0 as usize) else {
            return;
        };
        let src = Rect::new(0.0, 0.0, off.width as f32, off.height as f32);
        self.blit_offscreen_src(handle, src, dst_rect);
    }

    fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        if let Err(error) = self.try_blit_offscreen_src(handle, src_rect, dst_rect) {
            self.remember_frame_failure(error);
        }
    }

    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen target does not exist before blit",
            ));
        };
        let target = off.target;
        if let Some(active) = self.active_offscreen {
            if active == handle.0 {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "Picture offscreen target cannot blit into itself",
                ));
            }
            // The destination is the currently bound Picture target. Submit
            // its queued native/soft commands before the immediate source
            // blit so painter order remains destination commands → blit.
            // Flushing the main swapchain here would clear/submit the wrong
            // target and invert that order.
            self.try_flush_offscreen_paint(&ImageHandle(active))?;
        } else {
            // `blit_offscreen_target` is immediate on native APIs. Flush
            // clear, native work and any bounded CPU segment before it so
            // Picture does not leapfrog preceding painter-order commands.
            // The subsequent commands remain queued and are committed by the
            // same final present.
            self.flush_main_segment_before_ordered_boundary()?;
        }
        self.gpu_ctx
            .blit_offscreen_target(target, src_rect, dst_rect)
    }

    fn present(&mut self, damage: &DamageRegion) -> Result<(), Error> {
        if self.active_offscreen.is_some() {
            self.end_offscreen_paint();
        }
        if let Some(error) = self.surface.canvas.take_deferred_error() {
            self.surface.needs_gpu_clear = true;
            return Err(error);
        }
        if let Some(error) = self.frame_failure.take() {
            self.surface.needs_gpu_clear = true;
            return Err(error);
        }
        self.gpu_ctx.make_current()?;

        if self.surface.needs_gpu_clear {
            self.gpu_ctx.clear_render_target(0.0, 0.0, 0.0, 0.0)?;
            self.surface.needs_gpu_clear = false;
            self.surface.pending_clear_rects.clear();
        } else if !self.surface.pending_clear_rects.is_empty() {
            if let Err(err) = self.gpu_ctx.clear_rects(
                self.surface.width as f32,
                self.surface.height as f32,
                &self.surface.pending_clear_rects,
            ) {
                self.surface.needs_gpu_clear = true;
                return Err(err);
            }
            self.surface.pending_clear_rects.clear();
        }

        if let Err(err) = self.surface.canvas.submit_native(self.gpu_ctx.as_mut()) {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }
        if let Err(err) = self.surface.canvas.submit_soft(self.gpu_ctx.as_mut()) {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }

        let frame = PresentFrame::Swapchain {
            damage: damage.to_present_damage(),
        };
        if let Err(err) = self.gpu_ctx.present(&frame) {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }
        self.surface.canvas.commit_presented_frame();
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Drop for NativeGpuBackend {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            crate::core::log::error_fn(format!(
                "NativeGpuBackend: checked shutdown failed: {}",
                error.short_what()
            ));
        }
    }
}
