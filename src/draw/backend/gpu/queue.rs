//! GPU-native 命令入队 — gpu 子模块。
//!
//! 每种绘制原语的设备几何折叠与 [`PendingNativeOp`] 构造；不可表示的操作
//! 走软回退（[`super::canvas::NativeGpuCanvas2D::ensure_soft`]）或 typed 失败。

use std::sync::Arc;

use crate::core::{Point, Rect};
use crate::draw::Canvas2D;
use crate::draw::geometry::color::Color;
use crate::draw::geometry::path::{FillRule, Path, PathBuilder};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::tessellator;
use crate::draw::geometry::types::{BlendMode, GradientDirection, Radius, Transform};
use crate::draw::raster::rasterizer::core::align_rounded_rect;
// 引入所属 graphics backend Module 的全部入队原语。
use super::{
    GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSector, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect,
};

use super::canvas::NativeGpuCanvas2D;
use super::geometry::{
    convex_quad_is_valid, glyph_device_corners, quad_aabb, rounded_rect_path, scaled_corner_radii,
    scales_are_uniform, solid_mesh_from_affine_rect, stroke_options_for_transform,
};
use super::pending::{
    PendingNativeLinearGrad, PendingNativeMesh, PendingNativeOp, PendingNativeRadialGrad,
    PendingNativeRect, PendingNativeSector, PendingNativeShadow, PendingNativeStroke,
};

impl NativeGpuCanvas2D {
    pub(super) fn queue_solid_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        self.queue_solid_rect_inner(rect, color, radius, false);
    }

    /// 圆形保留调用方声明的同一设备圆心，不走按边界独立取整的圆角矩形对齐。
    pub(super) fn queue_solid_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let rect = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        self.queue_solid_rect_inner(rect, color, Some(Radius::uniform(r)), true);
    }

    fn queue_solid_rect_inner(
        &mut self,
        rect: Rect,
        color: Color,
        radius: Option<Radius>,
        preserve_circle_center: bool,
    ) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        // Alpha 与 SrcOver 继续使用普通 native/RHI pipeline。
        let native_src_over = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        // Additive 只在 adapter 明确暴露 retained RHI blend 时进入 shape queue。
        let native_additive =
            matches!(self.blend_mode, BlendMode::Additive) && self.native_caps.rhi_additive_blend;
        // 统一决定轴对齐矩形是否可以进入 native pending queue。
        let native_blend = native_src_over || native_additive;
        if self.soft_has_content || !self.native_caps.retained_color_target || !native_blend {
            self.with_soft_clip(|soft| {
                if preserve_circle_center {
                    soft.fill_circle(
                        rect.x + rect.w * 0.5,
                        rect.y + rect.h * 0.5,
                        rect.w * 0.5,
                        color,
                    );
                } else {
                    soft.fill_rect(rect, color, radius);
                }
            });
            self.mark_soft();
            return;
        }
        let scissor = self.scissor_aabb();
        let has_radius = radius
            .is_some_and(|rad| rad.tl != 0.0 || rad.tr != 0.0 || rad.br != 0.0 || rad.bl != 0.0);
        if let Some((device, scale)) = self.try_axis_aligned_device_rect(rect) {
            if device.w <= 0.0 || device.h <= 0.0 {
                return;
            }
            // 轴对齐各向异性：设备空间圆角用几何平均近似椭圆角，避免 typed 失败。
            let r = scaled_corner_radii(radius, scale);
            // 圆角矩形对齐物理像素网格：亚像素设备坐标下 SDF 弧线端点与像素
            // 中心错位，导致四角取整不对称（顶/底圆角视觉半径不一致）。
            let device = if !preserve_circle_center && r.iter().any(|radius| *radius > 0.0) {
                match align_rounded_rect(device) {
                    Some(device) => device,
                    None => return,
                }
            } else {
                device
            };
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
                    // lowering 据此选择 Shape 或 AdditiveShape。
                    additive: native_additive,
                    scissor,
                }));
            return;
        }
        // 当前 Additive 纵切只覆盖 shape SDF，不把仿射 mesh 错交给 SrcOver pipeline。
        if native_additive {
            // hybrid canvas 保持既有等价 soft fallback；GPU-only canvas 记录 typed failure。
            self.with_soft_clip(|soft| {
                if preserve_circle_center {
                    soft.fill_circle(
                        rect.x + rect.w * 0.5,
                        rect.y + rect.h * 0.5,
                        rect.w * 0.5,
                        color,
                    );
                } else {
                    soft.fill_rect(rect, color, radius);
                }
            });
            // 标记本次 soft 内容，供 retained owner 按 Additive segment 合成。
            self.mark_soft();
            // 禁止后续普通 mesh 分支丢失 Additive 语义。
            return;
        }
        // 一般仿射：直角矩形走三角形网格；圆角 SDF 不支持旋转/剪切。
        if !has_radius && self.native_caps.retained_color_target {
            let mesh = solid_mesh_from_affine_rect(
                rect,
                self.transform,
                self.offset_x,
                self.offset_y,
                self.solid_rgba(color),
            );
            self.pending_native
                .push(PendingNativeOp::SolidMesh(PendingNativeMesh {
                    mesh,
                    scissor,
                }));
            return;
        }
        // 变换圆角矩形使用共享路径 tessellation，保留真实圆角轮廓而不是压平为 AABB。
        if has_radius && self.native_caps.retained_color_target {
            // 路径保留逻辑空间半径，再由 queue_path_mesh 统一应用当前 affine transform。
            let path = rounded_rect_path(rect, radius);
            self.queue_path_mesh(&path, color, FillRule::NonZero, None);
            return;
        }
        self.soft_or_reject_transform("transformed rounded rect");
        if !self.gpu_only {
            self.with_soft_clip(|soft| {
                if preserve_circle_center {
                    soft.fill_circle(
                        rect.x + rect.w * 0.5,
                        rect.y + rect.h * 0.5,
                        rect.w * 0.5,
                        color,
                    );
                } else {
                    soft.fill_rect(rect, color, radius);
                }
            });
            self.mark_soft();
        }
    }

    pub(super) fn queue_stroke_rect(
        &mut self,
        rect: Rect,
        color: Color,
        line_width: f32,
        radius: Option<Radius>,
    ) {
        self.queue_stroke_rect_inner(rect, color, line_width, radius, false);
    }

    /// 圆形描边与填充共用未偏移的圆心，确保不同半径仍保持同心。
    pub(super) fn queue_stroke_circle(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        color: Color,
        line_width: f32,
    ) {
        let rect = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        self.queue_stroke_rect_inner(rect, color, line_width, Some(Radius::uniform(r)), true);
    }

    fn queue_stroke_rect_inner(
        &mut self,
        rect: Rect,
        color: Color,
        line_width: f32,
        radius: Option<Radius>,
        preserve_circle_center: bool,
    ) {
        let lw = line_width.max(0.0);
        if rect.w <= 0.0 || rect.h <= 0.0 || lw <= 0.0 {
            return;
        }
        // Alpha 与 SrcOver 继续使用普通 native/RHI pipeline。
        let native_src_over = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        // Additive 只在 adapter 明确暴露 retained RHI blend 时进入 shape queue。
        let native_additive =
            matches!(self.blend_mode, BlendMode::Additive) && self.native_caps.rhi_additive_blend;
        // 统一决定轴对齐描边是否可以进入 native pending queue。
        let native_blend = native_src_over || native_additive;
        if self.soft_has_content || !self.native_caps.retained_color_target || !native_blend {
            self.with_soft_clip(|soft| {
                if preserve_circle_center {
                    soft.stroke_circle(
                        rect.x + rect.w * 0.5,
                        rect.y + rect.h * 0.5,
                        rect.w * 0.5,
                        color,
                        lw,
                    );
                } else {
                    soft.stroke_rect(rect, color, lw, radius);
                }
            });
            self.mark_soft();
            return;
        }
        let Some((device, scale)) = self.try_axis_aligned_device_rect(rect) else {
            // 当前 Additive 纵切只覆盖轴对齐 shape SDF，不能误入 SrcOver 仿射 mesh。
            if native_additive {
                // hybrid canvas 保持等价 soft fallback；GPU-only canvas 记录 typed failure。
                self.with_soft_clip(|soft| {
                    if preserve_circle_center {
                        soft.stroke_circle(
                            rect.x + rect.w * 0.5,
                            rect.y + rect.h * 0.5,
                            rect.w * 0.5,
                            color,
                            lw,
                        );
                    } else {
                        soft.stroke_rect(rect, color, lw, radius);
                    }
                });
                // 记录目标相关 soft segment，供 retained owner 按原始顺序合成。
                self.mark_soft();
                // 禁止继续进入普通描边路径 tessellation。
                return;
            }
            // 任意仿射描边矩形转为闭合圆角路径，复用 solid mesh 与共享 stroker。
            if self.native_caps.retained_color_target {
                // 描边宽度保持逻辑值，由 queue_path_mesh 按当前 transform 处理。
                let path = rounded_rect_path(rect, radius);
                let options = StrokeOptions {
                    width: lw,
                    ..StrokeOptions::default()
                };
                self.queue_path_mesh(&path, color, FillRule::NonZero, Some(&options));
                return;
            }
            self.soft_or_reject_transform("non-axis-aligned stroke rect transform");
            if !self.gpu_only {
                self.with_soft_clip(|soft| {
                    if preserve_circle_center {
                        soft.stroke_circle(
                            rect.x + rect.w * 0.5,
                            rect.y + rect.h * 0.5,
                            rect.w * 0.5,
                            color,
                            lw,
                        );
                    } else {
                        soft.stroke_rect(rect, color, lw, radius);
                    }
                });
                self.mark_soft();
            }
            return;
        };
        if device.w <= 0.0 || device.h <= 0.0 {
            return;
        }
        let r = scaled_corner_radii(radius, scale);
        let stroke_w = lw * ((scale.0.abs() * scale.1.abs()).sqrt());
        // 圆角矩形对齐物理像素网格：亚像素设备坐标下 SDF 弧线端点与像素
        // 中心错位，导致四角取整不对称（顶/底圆角视觉半径不一致）。
        let device = if !preserve_circle_center && r.iter().any(|radius| *radius > 0.0) {
            match align_rounded_rect(device) {
                Some(device) => device,
                None => return,
            }
        } else {
            device
        };
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
                // lowering 据此选择 Shape 或 AdditiveShape。
                additive: native_additive,
                scissor: self.scissor_aabb(),
            }));
    }

    pub(super) fn queue_linear_gradient(
        &mut self,
        rect: Rect,
        ca: Color,
        cb: Color,
        dir: GradientDirection,
    ) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.retained_color_target || !native_blend {
            self.with_soft_clip(|soft| soft.fill_linear_gradient(rect, ca, cb, dir));
            self.mark_soft();
            return;
        }
        // 线性渐变直接保留逻辑矩形经过 affine 后的四角。
        let corners = glyph_device_corners(rect, self.transform, self.offset_x, self.offset_y);
        if !convex_quad_is_valid(&corners) {
            // 奇异变换不能稳定地映射单位渐变 quad。
            self.soft_or_reject_transform("degenerate linear gradient transform");
            if !self.gpu_only {
                self.with_soft_clip(|soft| soft.fill_linear_gradient(rect, ca, cb, dir));
                self.mark_soft();
            }
            return;
        }
        // AABB 仅服务于旧 native DTO 和裁剪诊断，shader 使用真实四角。
        let (min_x, min_y, max_x, max_y) = quad_aabb(corners);
        let device = Rect::new(min_x, min_y, max_x - min_x, max_y - min_y);
        if device.w <= 0.0 || device.h <= 0.0 || !device.x.is_finite() || !device.y.is_finite() {
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
                    corners,
                    color_a: self.rgba(ca),
                    color_b: self.rgba(cb),
                    dir: dir_u,
                },
                scissor: self.scissor_aabb(),
            }));
    }

    pub(super) fn queue_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        ir: f32,
        or: f32,
        ic: Color,
        oc: Color,
    ) {
        if or <= 0.0 {
            return;
        }
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.retained_color_target || !native_blend {
            self.with_soft_clip(|soft| soft.fill_radial_gradient(cx, cy, ir, or, ic, oc));
            self.mark_soft();
            return;
        }
        // 径向渐变把逻辑圆盘包围矩形映射为真实四角，shader 在局部坐标中求圆距。
        let bb = Rect::new(cx - or, cy - or, or * 2.0, or * 2.0);
        let corners = glyph_device_corners(bb, self.transform, self.offset_x, self.offset_y);
        if !convex_quad_is_valid(&corners) {
            // 奇异变换不能稳定地恢复局部圆盘坐标。
            self.soft_or_reject_transform("degenerate radial gradient transform");
            if !self.gpu_only {
                self.with_soft_clip(|soft| soft.fill_radial_gradient(cx, cy, ir, or, ic, oc));
                self.mark_soft();
            }
            return;
        }
        // 保留轴对齐缩放下旧 native DTO 的半径语义；仿射 RHI 使用四角和半径比值。
        let legacy_scale = self
            .try_axis_aligned_device_rect(bb)
            .and_then(|(_, scale)| scales_are_uniform(scale).then_some(scale.0.abs()))
            .unwrap_or(1.0);
        let center = self
            .transform
            .transform_point(Point::new(cx + self.offset_x, cy + self.offset_y));
        self.pending_native
            .push(PendingNativeOp::RadialGradient(PendingNativeRadialGrad {
                grad: GpuRadialGradient {
                    cx: center.x,
                    cy: center.y,
                    inner_r: ir.max(0.0) * legacy_scale,
                    outer_r: or * legacy_scale,
                    corners,
                    color_inner: self.rgba(ic),
                    color_outer: self.rgba(oc),
                },
                scissor: self.scissor_aabb(),
            }));
    }

    /// Queue a CPU-tessellated solid mesh, or soft-fallback on failure.
    pub(super) fn queue_path_mesh(
        &mut self,
        path: &Path,
        color: Color,
        fill_rule: FillRule,
        stroke: Option<&StrokeOptions>,
    ) {
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if self.soft_has_content || !self.native_caps.retained_color_target || !native_blend {
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

    /// Route strict-GPU ellipses through the shared path tessellator and solid-mesh
    /// submission path instead of allocating a CPU fallback surface.
    pub(super) fn queue_ellipse_mesh(&mut self, rect: Rect, color: Color) {
        if !rect.x.is_finite()
            || !rect.y.is_finite()
            || !rect.w.is_finite()
            || !rect.h.is_finite()
            || rect.w <= 0.0
            || rect.h <= 0.0
        {
            return;
        }

        let mut builder = PathBuilder::new();
        builder.ellipse(rect);
        self.queue_path_mesh(&builder.build(), color, FillRule::NonZero, None);
    }

    /// Route a circular sector through the shared path tessellator so strict
    /// GPU canvases never need a CPU fallback. Angles follow the historical
    /// Canvas2D contract: the filled sweep advances from `start_angle` toward
    /// `end_angle` in the positive direction, wrapping at one turn. A raw
    /// sweep of at least one turn is a full circle.
    pub(super) fn queue_sector_mesh(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
    ) {
        if !cx.is_finite()
            || !cy.is_finite()
            || !radius.is_finite()
            || !start_angle.is_finite()
            || !end_angle.is_finite()
            || radius <= 0.0
        {
            return;
        }

        let raw_sweep = end_angle - start_angle;
        if !raw_sweep.is_finite() {
            return;
        }
        let full_turn_epsilon = f32::EPSILON * 16.0;
        let is_full_turn = raw_sweep.abs() >= std::f32::consts::TAU - full_turn_epsilon;
        let sweep = if is_full_turn {
            std::f32::consts::TAU
        } else {
            raw_sweep.rem_euclid(std::f32::consts::TAU)
        };
        if sweep <= full_turn_epsilon {
            return;
        }

        // Normalizing first keeps trigonometry stable for callers that retain
        // an ever-increasing animation angle.
        let start = start_angle.rem_euclid(std::f32::consts::TAU);
        let native_blend = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver);
        if !self.soft_has_content && self.native_caps.retained_color_target && native_blend {
            let bounds = Rect::new(cx - radius, cy - radius, radius * 2.0, radius * 2.0);
            if let Some((_, scale)) = self.try_axis_aligned_device_rect(bounds) {
                if scales_are_uniform(scale) {
                    let center = self
                        .transform
                        .transform_point(Point::new(cx + self.offset_x, cy + self.offset_y));
                    self.pending_native
                        .push(PendingNativeOp::Sector(PendingNativeSector {
                            sector: GpuSector {
                                cx: center.x,
                                cy: center.y,
                                radius: radius * scale.0.abs(),
                                start_angle: start,
                                sweep_angle: sweep,
                                rgba: self.rgba(color),
                            },
                            scissor: self.scissor_aabb(),
                        }));
                    return;
                }
            }
        }
        if self.soft_has_content || !self.native_caps.retained_color_target || !native_blend {
            if self.gpu_only {
                self.reject_unsupported("sector GPU primitive or destination-dependent blend");
                return;
            }
            self.with_soft_clip(|soft| {
                if is_full_turn {
                    soft.fill_circle(cx, cy, radius, color);
                } else {
                    soft.fill_sector(cx, cy, radius, start, start + sweep, color);
                }
            });
            self.mark_soft();
            return;
        }

        let mut builder = PathBuilder::new();
        builder.arc(cx, cy, radius, start, start + sweep);
        if !is_full_turn {
            builder.line_to(cx, cy).close();
        }
        self.queue_path_mesh(&builder.build(), color, FillRule::NonZero, None);
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "shadow parameters mirror the canvas drawing contract"
    )]
    pub(super) fn queue_box_shadow(
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
        if self.soft_has_content || !self.native_caps.retained_color_target || !native_blend {
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
            // 圆角矩形对齐物理像素网格（与 fill/stroke 一致）：亚像素设备坐标下
            // SDF 弧线端点与像素中心错位导致四角取整不对称，blur=0 的阴影与
            // 填充同构同样受影响。blur>0 时模糊会掩盖差异，round 亦无副作用。
            let device_rect = if r.iter().any(|radius| *radius > 0.0) {
                match align_rounded_rect(device_rect) {
                    Some(rect) => rect,
                    None => return,
                }
            } else {
                device_rect
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
                    expanded.x, expanded.y, expanded.w, expanded.h,
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
}

// 验证 Additive 描边只在事实能力和几何证明同时成立时进入 native queue。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/draw/backend/gpu/queue__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
