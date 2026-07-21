//! API-neutral non-GL GPU-native raster backend.
//!
//! Hot Canvas2D paths (`fill_rect` / `fill_circle` / `stroke_rect` /
//! `stroke_circle`, axis-aligned `draw_line`, solid `blit_glyph`,
//! linear/radial gradients, fill-rule-aware `fill_path`,
//! cap/join-aware `stroke_path`, box/ambient shadow, and scaled image blit)
//! draw via [`IGraphicsContext`] operations advertised by [`NativeRasterCaps`].
//! Axis-aligned transforms are folded into device geometry; general affine
//! sharp fills use solid meshes. Every unsupported operation deterministically
//! soft-rasterizes into a CPU buffer and alpha-blits at present (or typed-fails
//! in GPU-only mode).

use std::any::Any;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::{DamageRegion, Errc, Error, Point, PresentDamageTracker, Rect};
use crate::draw::backend::traits::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::pipeline::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FrameEncoder, FrameEncoderError,
    FrameGlyphBlit, FrameRasterOp, FrameRect, FrameStrokeRect, ReferenceFrame,
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
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect, IGraphicsContext, NativeRasterCaps, OffscreenTargetId,
    PresentFrame, PresentMode, PresentTestResult, RasterMode, SoftFallbackTile,
};

const SOFT_FALLBACK_IDLE_PRESENT_GRACE: u8 = 2;
pub(crate) const SOFT_FALLBACK_IDLE_TIME_GRACE: Duration = Duration::from_millis(250);

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
    pub(crate) rect: GpuSolidRect,
    /// Logical scissor AABB (x, y, w, h).
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeStroke {
    pub(crate) rect: GpuStrokeRect,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeGlyph {
    pub(crate) glyph: GpuGlyphBlit,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeLinearGrad {
    pub(crate) rect: GpuLinearGradientRect,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeRadialGrad {
    pub(crate) grad: GpuRadialGradient,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeMesh {
    pub(crate) mesh: GpuSolidMesh,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) struct PendingNativeShadow {
    pub(crate) shadow: GpuBoxShadow,
    pub(crate) scissor: (i32, i32, i32, i32),
}

/// 严格 GPU image blit 规划结果：可见入队 / 不可见跳过 / 需 soft 或 typed 失败。
enum DirectImageBlit {
    Ready(GpuImageBlit),
    Culled,
    Unsupported,
}

pub(crate) struct PendingNativeImage {
    pub(crate) blit: GpuImageBlit,
    pub(crate) scissor: (i32, i32, i32, i32),
}

pub(crate) enum PendingNativeOp {
    SolidRect(PendingNativeRect),
    StrokeRect(PendingNativeStroke),
    Glyph(PendingNativeGlyph),
    LinearGradient(PendingNativeLinearGrad),
    RadialGradient(PendingNativeRadialGrad),
    SolidMesh(PendingNativeMesh),
    BoxShadow(PendingNativeShadow),
    ImageBlit(PendingNativeImage),
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
            Self::ImageBlit(op) => op.scissor,
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
                | (Self::ImageBlit(_), Self::ImageBlit(_))
        )
    }
}

/// Native Canvas2D. Compatibility callers may retain the historical soft
/// fallback; production GPU engines construct it in strict GPU-only mode.
pub struct NativeGpuCanvas2D {
    native_caps: NativeRasterCaps,
    gpu_only: bool,
    /// Allocated on first soft-path use (#105) — pure-native frames keep no CPU framebuffer.
    pub(crate) soft_fallback: Option<SharedRasterizer>,
    /// Soft buffer has content that must be composited (until full clear).
    pub(crate) soft_has_content: bool,
    /// At least one soft operation contributed to the current swapchain frame.
    /// Segment commits do not reset this: ordered Picture boundaries may split
    /// one frame into several submissions before the final present.
    soft_used_since_present: bool,
    /// Successful swapchain presents since this canvas last used its soft
    /// fallback. A short grace avoids allocation churn in alternating frames.
    soft_idle_presents: u8,
    /// Destination-dependent blend cannot be faithfully composed from a
    /// transparent CPU segment over native output.
    soft_uses_destination_blend: bool,
    /// Immediate Canvas2D calls that have no `Result` return channel record
    /// an error here. The frame boundary consumes it before any present.
    deferred_error: Option<Error>,
    /// Zero-length return target for the legacy `pixels_mut` method in strict
    /// GPU mode; it preserves the trait contract without allocating a CPU surface.
    rejected_pixels: [u32; 0],
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
        Self::new_with_mode(width, height, native_caps, false)
    }

    pub(crate) fn new_gpu_only(width: i32, height: i32, native_caps: NativeRasterCaps) -> Self {
        Self::new_with_mode(width, height, native_caps, true)
    }

    fn new_with_mode(
        width: i32,
        height: i32,
        native_caps: NativeRasterCaps,
        gpu_only: bool,
    ) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        Self {
            native_caps,
            gpu_only,
            soft_fallback: None,
            soft_has_content: false,
            soft_used_since_present: false,
            soft_idle_presents: 0,
            soft_uses_destination_blend: false,
            deferred_error: None,
            rejected_pixels: [],
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

    #[cfg(all(test, feature = "d3d11"))]
    pub(crate) fn pending_mesh_count(&self) -> usize {
        self.pending_native
            .iter()
            .filter(|op| matches!(op, PendingNativeOp::SolidMesh(_)))
            .count()
    }

    pub(crate) fn ensure_soft(&mut self) -> &mut SharedRasterizer {
        let width = self.surface_w;
        let height = self.surface_h;
        let deferred_error = &mut self.deferred_error;
        self.soft_fallback.get_or_insert_with(|| {
            let surface = match PixelSurface::try_new(width, height) {
                Ok(surface) => surface,
                Err(error) => {
                    if deferred_error.is_none() {
                        *deferred_error = Some(error);
                    }
                    PixelSurface::one_pixel()
                }
            };
            SharedRasterizer::new(surface)
        })
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
        self.soft_used_since_present = false;
        self.soft_idle_presents = 0;
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
        if self.gpu_only {
            self.reject_unsupported("CPU soft raster fallback in GPU-only mode");
            return;
        }
        self.soft_has_content = true;
        self.soft_used_since_present = true;
        self.soft_idle_presents = 0;
    }

    pub(crate) fn reset_for_repaint(&mut self) {
        let fallback_extent_mismatch = self.soft_fallback.as_ref().is_some_and(|soft| {
            soft.surface().width() != self.surface_w || soft.surface().height() != self.surface_h
        });
        if fallback_extent_mismatch {
            // Allocation failure installs a 1×1 safety surface. Do not let a
            // later repaint mistake that placeholder for a valid full target:
            // dropping it makes the next soft draw retry the typed allocation.
            self.soft_fallback = None;
        } else if let Some(soft) = self.soft_fallback.as_mut() {
            soft.surface_mut().clear_all();
            soft.reset_state_for_extent(self.surface_w, self.surface_h);
        }
        self.soft_has_content = false;
        self.soft_uses_destination_blend = false;
        self.deferred_error = None;
        self.pending_native.clear();
        self.clip_rect = Rect::new(0.0, 0.0, self.surface_w as f32, self.surface_h as f32);
        self.clip_stack.clear();
        self.opacity = 1.0;
        self.offset_x = 0.0;
        self.offset_y = 0.0;
        self.transform = Transform::identity();
        self.blend_mode = BlendMode::default();
        self.state_stack.clear();
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
        if self.gpu_only {
            self.reject_unsupported("CPU soft raster fallback in GPU-only mode");
            return;
        }
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
        if self.gpu_only {
            self.reject_unsupported("CPU soft raster fallback in GPU-only mode");
            return;
        }
        self.sync_fallback_state();
        let clip = self.clip_rect;
        let soft = self.ensure_soft();
        soft.push_clip_surface(clip);
        f(soft);
        soft.pop_clip();
    }

    fn queue_solid_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.solid_rects || !native_blend {
            self.with_soft_clip(|soft| soft.fill_rect(rect, color, radius));
            self.mark_soft();
            return;
        }
        let scissor = self.scissor_aabb();
        let has_radius = radius.is_some_and(|rad| {
            rad.tl != 0.0 || rad.tr != 0.0 || rad.br != 0.0 || rad.bl != 0.0
        });
        if let Some((device, scale)) = self.try_axis_aligned_device_rect(rect) {
            if device.w <= 0.0 || device.h <= 0.0 {
                return;
            }
            // 轴对齐各向异性：设备空间圆角用几何平均近似椭圆角，避免 typed 失败。
            let r = scaled_corner_radii(radius, scale);
            self.pending_native
                .push(PendingNativeOp::SolidRect(PendingNativeRect {
                    rect: GpuSolidRect {
                        x: device.x,
                        y: device.y,
                        w: device.w,
                        h: device.h,
                        rgba: self.solid_rgba(color),
                        radius: r,
                    },
                    scissor,
                }));
            return;
        }
        // 一般仿射：直角矩形走三角形网格；圆角 SDF 不支持旋转/剪切。
        if !has_radius && self.native_caps.solid_meshes {
            let mesh = solid_mesh_from_affine_rect(
                rect,
                self.transform,
                self.offset_x,
                self.offset_y,
                self.solid_rgba(color),
            );
            self.pending_native
                .push(PendingNativeOp::SolidMesh(PendingNativeMesh { mesh, scissor }));
            return;
        }
        self.soft_or_reject_transform("transformed rounded rect");
        if !self.gpu_only {
            self.with_soft_clip(|soft| soft.fill_rect(rect, color, radius));
            self.mark_soft();
        }
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
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.stroke_rects || !native_blend {
            self.with_soft_clip(|soft| soft.stroke_rect(rect, color, lw, radius));
            self.mark_soft();
            return;
        }
        let Some((device, scale)) = self.try_axis_aligned_device_rect(rect) else {
            self.soft_or_reject_transform("non-axis-aligned stroke rect transform");
            if !self.gpu_only {
                self.with_soft_clip(|soft| soft.stroke_rect(rect, color, lw, radius));
                self.mark_soft();
            }
            return;
        };
        if device.w <= 0.0 || device.h <= 0.0 {
            return;
        }
        let r = scaled_corner_radii(radius, scale);
        let stroke_w = lw * ((scale.0.abs() * scale.1.abs()).sqrt());
        self.pending_native
            .push(PendingNativeOp::StrokeRect(PendingNativeStroke {
                rect: GpuStrokeRect {
                    x: device.x,
                    y: device.y,
                    w: device.w,
                    h: device.h,
                    rgba: self.rgba(color),
                    radius: r,
                    line_width: stroke_w,
                },
                scissor: self.scissor_aabb(),
            }));
    }

    fn queue_linear_gradient(&mut self, rect: Rect, ca: Color, cb: Color, dir: GradientDirection) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.linear_gradients || !native_blend {
            self.with_soft_clip(|soft| soft.fill_linear_gradient(rect, ca, cb, dir));
            self.mark_soft();
            return;
        }
        let Some((device, _)) = self.try_axis_aligned_device_rect(rect) else {
            self.soft_or_reject_transform("non-axis-aligned linear gradient transform");
            if !self.gpu_only {
                self.with_soft_clip(|soft| soft.fill_linear_gradient(rect, ca, cb, dir));
                self.mark_soft();
            }
            return;
        };
        if device.w <= 0.0 || device.h <= 0.0 {
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
                    x: device.x,
                    y: device.y,
                    w: device.w,
                    h: device.h,
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
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.radial_gradients || !native_blend {
            self.with_soft_clip(|soft| soft.fill_radial_gradient(cx, cy, ir, or, ic, oc));
            self.mark_soft();
            return;
        }
        let identity = self.transform.m == Transform::identity().m;
        if identity {
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
            return;
        }
        // 径向在 GPU 上是圆；仅均匀轴对齐缩放可保持圆语义。
        let bb = Rect::new(cx - or, cy - or, or * 2.0, or * 2.0);
        let Some((_, scale)) = self.try_axis_aligned_device_rect(bb) else {
            self.soft_or_reject_transform("non-axis-aligned radial gradient transform");
            if !self.gpu_only {
                self.with_soft_clip(|soft| soft.fill_radial_gradient(cx, cy, ir, or, ic, oc));
                self.mark_soft();
            }
            return;
        };
        if !scales_are_uniform(scale) {
            self.soft_or_reject_transform("anisotropic radial gradient transform");
            if !self.gpu_only {
                self.with_soft_clip(|soft| soft.fill_radial_gradient(cx, cy, ir, or, ic, oc));
                self.mark_soft();
            }
            return;
        }
        let center = self.transform.transform_point(Point::new(
            cx + self.offset_x,
            cy + self.offset_y,
        ));
        let s = scale.0.abs();
        self.pending_native
            .push(PendingNativeOp::RadialGradient(PendingNativeRadialGrad {
                grad: GpuRadialGradient {
                    cx: center.x,
                    cy: center.y,
                    inner_r: ir.max(0.0) * s,
                    outer_r: or * s,
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
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.solid_meshes || !native_blend {
            if self.gpu_only {
                self.reject_unsupported("transformed path or destination-dependent path blend");
                return;
            }
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
        let identity = self.transform.m == Transform::identity().m;
        let path_for_tess = if identity && ox == 0.0 && oy == 0.0 {
            None
        } else if identity {
            Some(path.translated(ox, oy))
        } else {
            let composed = self.transform.concat(Transform::translate(ox, oy));
            Some(path.transformed(composed))
        };
        let path_for_tess = path_for_tess.as_ref().unwrap_or(path);
        let verts = if let Some(opts) = stroke {
            // 均匀轴对齐缩放时把线宽折进 stroke options；各向异性 / 旋转由路径变换近似。
            let scaled_opts = stroke_options_for_transform(opts, self.transform);
            tessellator::tessellate_stroke(path_for_tess, &scaled_opts)
        } else {
            tessellator::tessellate_fill(path_for_tess, fill_rule)
        };
        let Some(verts) = verts else {
            if self.gpu_only {
                self.reject_unsupported("path tessellation failure");
                return;
            }
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
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.box_shadows || !native_blend {
            if self.gpu_only {
                self.reject_unsupported("destination-dependent shadow blend");
                return;
            }
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
        let identity = self.transform.m == Transform::identity().m;
        let axis_aligned = if identity {
            Some((
                Rect::new(
                    rect.x + self.offset_x,
                    rect.y + self.offset_y,
                    rect.w,
                    rect.h,
                ),
                ox,
                oy,
                blur.max(0.0),
                blur.max(0.0),
                rad,
            ))
        } else if let Some((device_rect, (sx, sy))) = self.try_axis_aligned_device_rect(rect) {
            if sx.is_finite() && sy.is_finite() && sx != 0.0 && sy != 0.0 {
                let mapped_rad = rad.map(|radius| {
                    let scaled = scaled_corner_radii(Some(radius), (sx, sy));
                    Radius {
                        tl: scaled[0],
                        tr: scaled[1],
                        br: scaled[2],
                        bl: scaled[3],
                    }
                });
                Some((
                    device_rect,
                    ox * sx,
                    oy * sy,
                    blur.max(0.0) * sx.abs(),
                    blur.max(0.0) * sy.abs(),
                    mapped_rad,
                ))
            } else {
                None
            }
        } else {
            None
        };
        let shadow = if let Some((
            device_rect,
            mapped_ox,
            mapped_oy,
            mapped_blur_x,
            mapped_blur_y,
            mapped_rad,
        )) = axis_aligned
        {
            let r = match mapped_rad {
                Some(radius) => [radius.tl, radius.tr, radius.br, radius.bl],
                None => [0.0; 4],
            };
            let expanded = Rect::new(
                device_rect.x + mapped_ox - mapped_blur_x,
                device_rect.y + mapped_oy - mapped_blur_y,
                device_rect.w + mapped_blur_x * 2.0,
                device_rect.h + mapped_blur_y * 2.0,
            );
            GpuBoxShadow {
                x: device_rect.x,
                y: device_rect.y,
                w: device_rect.w,
                h: device_rect.h,
                offset_x: mapped_ox,
                offset_y: mapped_oy,
                blur_x: mapped_blur_x,
                blur_y: mapped_blur_y,
                rgba: self.rgba(color),
                radius: r,
                ambient,
                corners: GpuGlyphBlit::axis_aligned_corners(
                    expanded.x,
                    expanded.y,
                    expanded.w,
                    expanded.h,
                ),
            }
        } else {
            // 旋转 / 剪切：逻辑空间 SDF，设备四角经仿射映射。
            let [a, b, _, c, d, _] = self.transform.m;
            if !a.is_finite()
                || !b.is_finite()
                || !c.is_finite()
                || !d.is_finite()
                || (a * d - b * c).abs() < 1e-12
            {
                if self.gpu_only {
                    self.reject_unsupported("non-invertible shadow transform");
                    return;
                }
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
            let blur = blur.max(0.0);
            let body = Rect::new(
                rect.x + self.offset_x,
                rect.y + self.offset_y,
                rect.w,
                rect.h,
            );
            let expanded = Rect::new(
                body.x + ox - blur,
                body.y + oy - blur,
                body.w + blur * 2.0,
                body.h + blur * 2.0,
            );
            let map = |x: f32, y: f32| {
                let point = self.transform.transform_point(Point::new(x, y));
                [point.x, point.y]
            };
            let corners = [
                map(expanded.x, expanded.y),
                map(expanded.x + expanded.w, expanded.y),
                map(expanded.x + expanded.w, expanded.y + expanded.h),
                map(expanded.x, expanded.y + expanded.h),
            ];
            let r = match rad {
                Some(radius) => [radius.tl, radius.tr, radius.br, radius.bl],
                None => [0.0; 4],
            };
            GpuBoxShadow {
                x: body.x,
                y: body.y,
                w: body.w,
                h: body.h,
                offset_x: ox,
                offset_y: oy,
                blur_x: blur,
                blur_y: blur,
                rgba: self.rgba(color),
                radius: r,
                ambient,
                corners,
            }
        };
        self.pending_native
            .push(PendingNativeOp::BoxShadow(PendingNativeShadow {
                shadow,
                scissor: self.scissor_aabb(),
            }));
    }

    /// 轴对齐（无剪切/旋转）变换下把逻辑矩形映射到设备空间。
    fn try_axis_aligned_device_rect(&self, rect: Rect) -> Option<(Rect, (f32, f32))> {
        let [a, b, _, c, d, _] = self.transform.m;
        if b != 0.0 || c != 0.0 || !a.is_finite() || !d.is_finite() || a == 0.0 || d == 0.0 {
            return None;
        }
        let offset = Rect::new(
            rect.x + self.offset_x,
            rect.y + self.offset_y,
            rect.w,
            rect.h,
        );
        let device = if self.transform.m == Transform::identity().m {
            offset
        } else {
            self.transform.transform_rect(offset)
        };
        if !device.x.is_finite()
            || !device.y.is_finite()
            || !device.w.is_finite()
            || !device.h.is_finite()
        {
            return None;
        }
        Some((device, (a, d)))
    }

    fn soft_or_reject_transform(&mut self, operation: &str) {
        if self.gpu_only {
            self.reject_unsupported(operation);
        }
    }

    fn rgba(&self, color: Color) -> [f32; 4] {
        [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            (color.a as f32 / 255.0) * self.opacity,
        ]
    }

    fn solid_rgba(&self, color: Color) -> [f32; 4] {
        let color = crate::draw::rasterizer::color_with_premultiplied_opacity(color, self.opacity);
        [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
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

    /// 严格 GPU 路径：整数像素源 crop；目标可 1:1 或缩放；位置允许亚像素。
    /// 轴对齐仿射（平移/缩放，无旋转/剪切）经 [`Self::try_axis_aligned_device_rect`]
    /// 映射到设备空间；裁剪交给 scissor。SrcOver 与 Additive 均可入队。
    ///
    /// `Culled` 表示完全不可见（屏外 / 空 clip），gpu-only 必须 no-op，不得当成未实现。
    fn try_queue_direct_image_blit(
        &self,
        pixels: &[u32],
        source_width: i32,
        source_rect: Rect,
        destination_rect: Rect,
    ) -> DirectImageBlit {
        if !self.opacity.is_finite() || self.opacity <= 0.0 {
            return DirectImageBlit::Culled;
        }
        let native_blend = matches!(
            self.blend_mode,
            BlendMode::Alpha | BlendMode::SrcOver | BlendMode::Additive
        );
        if !native_blend {
            return DirectImageBlit::Unsupported;
        }
        let Ok(source_stride) = usize::try_from(source_width) else {
            return DirectImageBlit::Unsupported;
        };
        if source_stride == 0 {
            return DirectImageBlit::Culled;
        }
        let Ok(source_height) = i32::try_from(pixels.len() / source_stride) else {
            return DirectImageBlit::Unsupported;
        };
        let Some(source) = rect_to_integer_frame(source_rect) else {
            return DirectImageBlit::Unsupported;
        };
        if source.width <= 0 || source.height <= 0 {
            return DirectImageBlit::Culled;
        }
        if !frame_within(source, source_width, source_height) {
            return DirectImageBlit::Unsupported;
        }
        if !destination_rect.w.is_finite()
            || !destination_rect.h.is_finite()
            || destination_rect.w <= 0.0
            || destination_rect.h <= 0.0
        {
            return DirectImageBlit::Culled;
        }
        let Some((device_dst, _)) = self.try_axis_aligned_device_rect(destination_rect) else {
            return DirectImageBlit::Unsupported;
        };
        if device_dst.w <= 0.0 || device_dst.h <= 0.0 {
            return DirectImageBlit::Culled;
        }
        if device_dst.x + device_dst.w <= 0.0
            || device_dst.y + device_dst.h <= 0.0
            || device_dst.x >= self.surface_w as f32
            || device_dst.y >= self.surface_h as f32
        {
            // 完全落在表面外：跳过上传，不是能力缺口。
            return DirectImageBlit::Culled;
        }

        let identity = self.transform.m == Transform::identity().m;
        let one_to_one = destination_rect.w == source.width as f32
            && destination_rect.h == source.height as f32;
        let (blit_x, blit_y, blit_w, blit_h, crop) = if identity && one_to_one {
            let dest_x = device_dst.x;
            let dest_y = device_dst.y;
            let integer_placement = self.offset_x.fract() == 0.0
                && self.offset_y.fract() == 0.0
                && dest_x.fract() == 0.0
                && dest_y.fract() == 0.0;
            if integer_placement {
                let destination = IntegerFrame {
                    x: dest_x as i32,
                    y: dest_y as i32,
                    width: source.width,
                    height: source.height,
                };
                // 与 clip 求交；表面边界一并收窄，避免部分越界被误判为未实现。
                let mut left = destination.x.max(0);
                let mut top = destination.y.max(0);
                let mut right = destination
                    .x
                    .saturating_add(destination.width)
                    .min(self.surface_w);
                let mut bottom = destination
                    .y
                    .saturating_add(destination.height)
                    .min(self.surface_h);
                if let Some(clip) = rect_to_integer_frame(self.clip_rect) {
                    left = left.max(clip.x);
                    top = top.max(clip.y);
                    right = right.min(clip.x.saturating_add(clip.width));
                    bottom = bottom.min(clip.y.saturating_add(clip.height));
                }
                if left >= right || top >= bottom {
                    return DirectImageBlit::Culled;
                }
                let clipped_dst = IntegerFrame {
                    x: left,
                    y: top,
                    width: right - left,
                    height: bottom - top,
                };
                let clipped_src = IntegerFrame {
                    x: source.x.saturating_add(left - destination.x),
                    y: source.y.saturating_add(top - destination.y),
                    width: clipped_dst.width,
                    height: clipped_dst.height,
                };
                (
                    clipped_dst.x as f32,
                    clipped_dst.y as f32,
                    clipped_dst.width as f32,
                    clipped_dst.height as f32,
                    clipped_src,
                )
            } else {
                // 亚像素落点：保留完整源 crop，裁剪交给 GPU scissor。
                (
                    dest_x,
                    dest_y,
                    source.width as f32,
                    source.height as f32,
                    source,
                )
            }
        } else {
            // 缩放或轴对齐 view 变换：上传完整源 crop，设备尺寸由 GPU 纹理采样。
            (
                device_dst.x,
                device_dst.y,
                device_dst.w,
                device_dst.h,
                source,
            )
        };

        let Some(pixel_count) = usize::try_from(i64::from(crop.width) * i64::from(crop.height)).ok()
        else {
            return DirectImageBlit::Unsupported;
        };
        let mut retained = Vec::new();
        if retained.try_reserve_exact(pixel_count).is_err() {
            return DirectImageBlit::Unsupported;
        }
        let copy_width = crop.width as usize;
        for y in crop.y..crop.y + crop.height {
            let row = y as usize * source_stride + crop.x as usize;
            retained.extend_from_slice(&pixels[row..row + copy_width]);
        }
        DirectImageBlit::Ready(GpuImageBlit {
            x: blit_x,
            y: blit_y,
            w: blit_w,
            h: blit_h,
            opacity: self.opacity.clamp(0.0, 1.0),
            additive: matches!(self.blend_mode, BlendMode::Additive),
            pixels: Arc::<[u32]>::from(retained),
            pixel_w: crop.width as u32,
            pixel_h: crop.height as u32,
        })
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
                PendingNativeOp::ImageBlit(_) => {
                    let batch = self.pending_native[start..end]
                        .iter()
                        .map(|op| match op {
                            PendingNativeOp::ImageBlit(op) => op.blit.clone(),
                            _ => unreachable!("native batch kind changed"),
                        })
                        .collect::<Vec<_>>();
                    gpu_ctx.draw_image_blits(vw, vh, Some(scissor), &batch)?;
                }
            }
            start = end;
        }
        Ok(())
    }

    pub(crate) fn current_blend_mode(&self) -> BlendMode {
        self.blend_mode
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

    /// Completes one successful swapchain present, then ages this canvas's idle
    /// full-size soft allocation.
    pub(crate) fn finish_presented_frame(&mut self) -> bool {
        self.commit_presented_frame();
        self.age_soft_fallback_after_present()
    }

    /// Ages an offscreen canvas at the final swapchain success boundary without
    /// committing any unflushed Picture commands.
    pub(crate) fn age_soft_fallback_after_present(&mut self) -> bool {
        if self.soft_used_since_present {
            self.soft_used_since_present = false;
            self.soft_idle_presents = 0;
            return true;
        }
        if self.soft_fallback.is_none() || self.soft_has_content || self.deferred_error.is_some() {
            return false;
        }
        self.soft_idle_presents = self.soft_idle_presents.saturating_add(1);
        if self.soft_idle_presents >= SOFT_FALLBACK_IDLE_PRESENT_GRACE {
            self.soft_fallback = None;
            self.soft_idle_presents = 0;
        }
        false
    }

    fn release_idle_soft_fallback(&mut self) {
        if self.soft_has_content || self.soft_used_since_present || self.deferred_error.is_some() {
            return;
        }
        self.soft_fallback = None;
        self.soft_idle_presents = 0;
    }

    /// A successful Picture flush has copied every soft pixel into its durable
    /// GPU render target. Unlike the swapchain canvas, an eligible Picture is
    /// static by policy, so keeping a second full-size CPU surface for a future
    /// repaint is not worth the resident memory.
    fn release_committed_picture_staging(&mut self) {
        if self.soft_has_content || !self.pending_native.is_empty() || self.deferred_error.is_some()
        {
            return;
        }
        self.soft_fallback = None;
        self.soft_used_since_present = false;
        self.soft_idle_presents = 0;
        self.soft_uses_destination_blend = false;
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
            self.reject_unsupported("ellipse GPU primitive");
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
        if self.gpu_only {
            self.reject_unsupported("sector GPU primitive");
            return;
        }
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
        analytic_coverage: Option<std::sync::Arc<[u8]>>,
        w: usize,
        h: usize,
        color: Color,
    ) {
        if w == 0 || h == 0 || !crate::draw::font::glyph_outline::is_outline_edges(mesh.as_ref()) {
            return;
        }
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.glyphs || !native_blend {
            if self.gpu_only {
                self.reject_unsupported("destination-dependent glyph blend");
                return;
            }
            // soft：解析 AA coverage（1:1 契约）；优先复用缓存。
            self.sync_fallback_state();
            let _soft_clip = self.clip_rect;
            self.ensure_soft().push_clip(_soft_clip);
            self.ensure_soft().blit_glyph_outline_shared(
                x,
                y,
                mesh,
                analytic_coverage,
                w,
                h,
                color,
            );
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
        // 近 1:1：解析 AA → R8 atlas（与 soft 同锐利度）；缩放/仿射仍走 MSDF。
        if outline_uses_analytic_r8(self.transform, device_w, device_h, w, h) {
            let expected = w.saturating_mul(h);
            let coverage = analytic_coverage
                .filter(|c| c.len() >= expected)
                .or_else(|| {
                    crate::draw::font::glyph_outline::coverage_from_edges(mesh.as_ref(), w, h)
                        .map(std::sync::Arc::<[u8]>::from)
                });
            let Some(coverage) = coverage else {
                return;
            };
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

mod geometry;
use geometry::{
    frame_within, glyph_device_corners, outline_uses_analytic_r8, quad_aabb, rect_to_integer_frame,
    scaled_corner_radii, scales_are_uniform, solid_mesh_from_affine_rect,
    stroke_options_for_transform, uniform_transform_scale, IntegerFrame,
};

mod backend;

pub use backend::{NativeGpuBackend, NativeGpuDrawSurface};
