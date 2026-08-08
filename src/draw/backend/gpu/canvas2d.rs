//! [`NativeGpuCanvas2D`] 的 `Canvas2D` 实现 — gpu 子模块。
//!
//! 热路径直接入队 native 命令；unsupported 操作确定性软光栅化并在 present
//! 时 alpha-blit（GPU-only 模式 typed 失败）。

use std::sync::Arc;

use crate::core::{Point, Rect};
use crate::draw::geometry::color::Color;
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::types::{BlendMode, GradientDirection, Radius, Transform};
use crate::draw::Canvas2D;
use crate::native::present::{GpuGlyphBlit, GpuSolidMesh, GpuSolidRect};

use super::canvas::NativeGpuCanvas2D;
use super::geometry::{
    glyph_device_corners, outline_uses_area_r8, quad_aabb, uniform_transform_scale,
};
use super::pending::StateSnapshot;
use super::pending::{
    DirectImageBlit, PendingNativeGlyph, PendingNativeImage,
    PendingNativeMesh, PendingNativeOp, PendingNativeRect,
    PendingNativeScroll,
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
        // hybrid 与 GPU-only 共用同一条有界 ellipse path tessellation lowering。
        self.queue_ellipse_mesh(rect, color);
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
                        // 轴对齐线段仍只由普通 SrcOver 快路入队。
                        additive: false,
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

    fn push_clip_path(&mut self, path: &Path) {
        // GPU-only 没有 stencil/path-mask 原语时继续保持确定性 typed failure。
        if self.gpu_only {
            self.reject_path_clip();
            return;
        }
        // 计算与 native path lowering 相同的设备空间路径和保守边界。
        let composed = self
            .transform
            .concat(Transform::translate(self.offset_x, self.offset_y));
        let device_path = path.transformed(composed);
        let path_bounds = device_path.bounds();
        // 先同步状态，再让共享 soft renderer 建立真正的逐像素 path mask。
        self.sync_fallback_state();
        let result = {
            let soft = self.ensure_soft();
            soft.try_push_clip_path(path)
        };
        // 失败时不入 native clip 栈，帧边界会消费该 typed error。
        if let Err(error) = result {
            if self.deferred_error.is_none() {
                self.deferred_error = Some(error);
            }
            return;
        }
        // 路径 clip 一旦生效，后续绘制统一走 soft staging，保证 native primitive 不越过 mask。
        self.clip_stack.push(self.clip_rect);
        self.clip_kind_stack.push(true);
        if let Some(bounds) = path_bounds.and_then(|bounds| self.clip_rect.intersect(&bounds)) {
            self.clip_rect = bounds;
        } else {
            self.clip_rect = Rect::zero();
        }
        self.mark_soft();
    }

    fn save(&mut self) {
        // 已存在 soft renderer 时同步保存其 path mask 与内部 clip 栈。
        let soft_was_present = self.soft_fallback.is_some();
        if soft_was_present {
            if let Some(soft) = self.soft_fallback.as_mut() {
                soft.save();
            }
        }
        // GPU canvas 额外保存裁剪栈长度，保持 native/soft 两套栈的 pop 边界一致。
        self.state_stack.push(StateSnapshot {
            clip_rect: self.clip_rect,
            clip_stack_len: self.clip_stack.len(),
            clip_kind_stack_len: self.clip_kind_stack.len(),
            soft_was_present,
            opacity: self.opacity,
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            transform: self.transform,
            blend_mode: self.blend_mode,
        });
    }

    fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            // 保存快照决定 restore 是否需要弹出 soft renderer 的状态栈。
            let soft_was_present = state.soft_was_present;
            self.clip_rect = state.clip_rect;
            self.clip_stack.truncate(state.clip_stack_len);
            self.clip_kind_stack.truncate(state.clip_kind_stack_len);
            self.opacity = state.opacity;
            self.offset_x = state.offset_x;
            self.offset_y = state.offset_y;
            self.transform = state.transform;
            self.blend_mode = state.blend_mode;
            // soft 在 save 前存在时恢复完整 path mask；否则只重置其状态而保留已绘制像素。
            if soft_was_present {
                if let Some(soft) = self.soft_fallback.as_mut() {
                    soft.restore();
                }
            } else if let Some(soft) = self.soft_fallback.as_mut() {
                soft.reset_state_for_extent(self.surface_w, self.surface_h);
                soft.set_transform(self.transform);
                soft.set_opacity(self.opacity);
                soft.set_blend_mode(self.blend_mode);
                soft.set_offset(self.offset_x, self.offset_y);
            }
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
        self.clip_kind_stack.push(false);
        if let Some(intersection) = self.clip_rect.intersect(&rect) {
            self.clip_rect = intersection;
        } else {
            self.clip_rect = Rect::zero();
        }
    }

    fn pop_clip(&mut self) {
        // 只有路径裁剪在 soft renderer 中拥有持久 mask，需要成对弹出。
        let was_path = self.clip_kind_stack.pop().unwrap_or(false);
        if was_path {
            if let Some(soft) = self.soft_fallback.as_mut() {
                soft.pop_clip();
            }
        }
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
        // 测试直写像素也必须先建立与当前 blend 一致的 soft 段。
        self.prepare_soft_segment(self.blend_mode);
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

    fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32) {
        // 位移和视口必须可稳定转换到同一逻辑像素网格。
        if !dx.is_finite()
            || !dy.is_finite()
            || !viewport.x.is_finite()
            || !viewport.y.is_finite()
            || !viewport.w.is_finite()
            || !viewport.h.is_finite()
            || viewport.w <= 0.0
            || viewport.h <= 0.0
        {
            // Canvas2D 无 Result 通道，延迟到帧边界返回 typed error。
            self.reject_unsupported("scroll-region with invalid geometry");
            return;
        }
        // 与 CPU/shared rasterizer 保持相同的取整语义。
        let dx = dx.round();
        let dy = dy.round();
        // 零位移是严格 no-op，不制造无意义的 RHI boundary。
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        // 记录目标相关边界，后续由保序 mixed RHI lowering 执行 TextureMove。
        self.pending_native
            .push(PendingNativeOp::ScrollCopy(PendingNativeScroll {
                viewport,
                dx: dx as i32,
                dy: dy as i32,
            }));
    }
}

// 验证原生 Canvas2D scroll 会保留为 ordered native boundary。
#[cfg(test)]
mod tests {
    // 引入当前 canvas、能力表和 pending scroll 枚举。
    use super::super::canvas::NativeGpuCanvas2D;
    use super::super::pending::PendingNativeOp;
    use crate::core::Rect;
    use crate::draw::geometry::path::PathBuilder;
    // 引入 Additive 分段与圆角矩形测试需要的公开几何类型。
    use crate::draw::geometry::types::{BlendMode, Radius};
    use crate::draw::Canvas2D;
    use crate::draw::Color;
    use crate::native::present::NativeRasterCaps;

    // 正常整数 scroll 不应在 Canvas2D 入口被降级为 deferred error。
    #[test]
    fn records_native_scroll_boundary() {
        // 使用 GPU-only canvas，避免测试意外创建 CPU soft surface。
        let mut canvas = NativeGpuCanvas2D::new_gpu_only(64, 48, NativeRasterCaps::default());
        // 记录 source = viewport + delta、destination = viewport 的 scroll。
        canvas.scroll_region(Rect::new(8.0, 6.0, 32.0, 24.0), 3.0, -2.0);
        // 验证队列保留了目标相关操作，而不是即时拒绝。
        assert!(matches!(
            canvas.pending_native.as_slice(),
            [PendingNativeOp::ScrollCopy(scroll)]
                if scroll.viewport == Rect::new(8.0, 6.0, 32.0, 24.0)
                    && scroll.dx == 3
                    && scroll.dy == -2
        ));
        // 正常记录不应产生延迟错误。
        assert!(canvas.take_deferred_error().is_none());
    }

    // 非有限几何必须保留 typed failure，不能把 NaN 转成可执行 copy。
    #[test]
    fn rejects_invalid_native_scroll_geometry() {
        // 使用 GPU-only canvas 只验证入口审计，不触发任何 adapter 调用。
        let mut canvas = NativeGpuCanvas2D::new_gpu_only(16, 16, NativeRasterCaps::default());
        // 无穷 viewport 被延迟记录为明确的 NotImplemented 错误。
        canvas.scroll_region(Rect::new(f32::INFINITY, 0.0, 4.0, 4.0), 1.0, 0.0);
        // 验证没有生成可能污染 retained target 的 pending operation。
        assert!(canvas.pending_native.is_empty());
        // 验证错误仍保留在最终 present 边界消费。
        let Some(error) = canvas.take_deferred_error() else {
            // 非法 scroll 必须留下可消费的错误。
            panic!("invalid scroll must record an error");
        };
        assert_eq!(error.code(), crate::core::Errc::NotImplemented);
    }

    // hybrid canvas 在具备 solid mesh 时应把椭圆保留为 GPU native mesh。
    #[test]
    fn hybrid_ellipse_uses_shared_mesh_lowering() {
        // 只打开 ellipse 依赖的共享 mesh 能力，保持测试边界最小。
        let caps = NativeRasterCaps {
            solid_meshes: true,
            ..NativeRasterCaps::default()
        };
        // 使用 hybrid canvas 验证非 GPU-only 入口也能复用相同 lowering。
        let mut canvas = NativeGpuCanvas2D::new(64, 48, caps);
        // 记录一个有限的椭圆填充操作。
        canvas.fill_ellipse(Rect::new(8.0, 6.0, 24.0, 18.0), Color::red());
        // 椭圆应进入 solid mesh，且不分配 soft staging。
        assert!(matches!(
            canvas.pending_native.as_slice(),
            [PendingNativeOp::SolidMesh(_)]
        ));
        // 没有发生 CPU fallback allocation。
        assert!(canvas.soft_fallback.is_none());
    }

    // 生产 RHI capability 应让 Additive 圆角矩形绕过 CPU staging。
    #[test]
    fn additive_rounded_rect_uses_native_shape_when_capability_is_explicit() {
        // 只启用当前纵切需要的 shape 与 Additive RHI 事实能力。
        let caps = NativeRasterCaps {
            // 允许轴对齐实心矩形进入 native queue。
            solid_rects: true,
            // 声明 retained RHI 可以执行 Additive pipeline。
            rhi_additive_blend: true,
            // 其余能力保持关闭，避免测试依赖无关图元。
            ..NativeRasterCaps::default()
        };
        // 使用 hybrid canvas，确保若能力分流错误就会真实分配 soft staging。
        let mut canvas = NativeGpuCanvas2D::new(32, 24, caps);
        // 切换到 destination-dependent Additive 语义。
        canvas.set_blend_mode(BlendMode::Additive);
        // 记录一个轴对齐圆角矩形。
        canvas.fill_rect(
            // 使用有限正矩形覆盖 shape SDF 入队。
            Rect::new(4.0, 3.0, 12.0, 8.0),
            // 使用不透明红色验证正常颜色载荷。
            Color::red(),
            // 非零圆角确保同一路径覆盖 rounded shape。
            Some(Radius::uniform(2.0)),
        );
        // pending queue 必须显式保留 Additive 事实。
        assert!(matches!(
            canvas.pending_native.as_slice(),
            [PendingNativeOp::SolidRect(rect)] if rect.additive
        ));
        // 直达 RHI shape 时不得创建 CPU surface。
        assert!(canvas.soft_fallback.is_none());
        // native 内容不能同时伪装成 soft segment。
        assert!(!canvas.soft_has_content);
    }

    // 未声明 RHI Additive 的 adapter 必须保留既有等价 soft fallback。
    #[test]
    fn additive_rect_without_rhi_capability_stays_in_soft_segment() {
        // 仅打开普通 solid rect，刻意不声明 Additive RHI 能力。
        let caps = NativeRasterCaps {
            // 证明分流只受可选 blend 能力控制，而不是缺少矩形能力。
            solid_rects: true,
            // 其余能力包括 rhi_additive_blend 保持默认 false。
            ..NativeRasterCaps::default()
        };
        // 使用允许 soft fallback 的 hybrid canvas。
        let mut canvas = NativeGpuCanvas2D::new(20, 12, caps);
        // 请求 Additive 矩形。
        canvas.set_blend_mode(BlendMode::Additive);
        // 绘制有限直角矩形。
        canvas.fill_rect(Rect::new(2.0, 2.0, 6.0, 4.0), Color::green(), None);
        // 不能把没有 RHI 事实支撑的操作放入 native queue。
        assert!(canvas.pending_native.is_empty());
        // 等价 CPU staging 必须存在。
        assert!(canvas.soft_fallback.is_some());
        // 快照应只包含一个 Additive soft segment。
        let segments = canvas.packed_soft_segments();
        // 单次绘制不能产生额外段。
        assert_eq!(segments.len(), 1);
        // 段的目标 blend 必须保持 Additive。
        assert!(segments[0].additive);
    }

    // hybrid path clip 应建立 soft mask，并让随后绘制受 mask 约束。
    #[test]
    fn hybrid_path_clip_uses_shared_soft_mask() {
        // 使用普通 hybrid canvas，验证路径裁剪不再被直接拒绝。
        let mut canvas = NativeGpuCanvas2D::new(32, 32, NativeRasterCaps::default());
        // 构造一个左上三角形路径。
        let mut builder = PathBuilder::new();
        builder
            .move_to(1.0, 1.0)
            .line_to(20.0, 1.0)
            .line_to(1.0, 20.0)
            .close();
        let path = builder.build();
        // 入栈后应分配共享 soft renderer，而不是产生 deferred error。
        canvas.push_clip_path(&path);
        assert!(canvas.soft_fallback.is_some());
        assert!(canvas.take_deferred_error().is_none());
        // 绘制整块矩形，结果只应出现在三角形内部。
        canvas.fill_rect(Rect::new(0.0, 0.0, 32.0, 32.0), Color::red(), None);
        let pixels = canvas.pixels();
        assert!(pixels[2 * 32 + 2] != 0);
        assert_eq!(pixels[24 * 32 + 24], 0);
        // 弹出路径裁剪后，后续绘制应恢复矩形 clip 的完整范围。
        canvas.pop_clip();
        canvas.fill_rect(Rect::new(24.0, 24.0, 4.0, 4.0), Color::blue(), None);
        assert!(canvas.pixels()[25 * 32 + 25] != 0);
    }

    // 连续 soft 操作必须按 SrcOver/Additive 切换封口并保持 painter order。
    #[test]
    fn soft_blend_changes_seal_ordered_segments() {
        // 默认能力强制三个矩形都进入共享 soft renderer。
        let mut canvas = NativeGpuCanvas2D::new(16, 8, NativeRasterCaps::default());
        // 首段使用默认 SrcOver 等价语义。
        canvas.fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::red(), None);
        // 第二段切换到 destination-dependent Additive。
        canvas.set_blend_mode(BlendMode::Additive);
        // 非重叠像素让每段都保留独立、可检查的紧密 tile。
        canvas.fill_rect(Rect::new(5.0, 1.0, 2.0, 2.0), Color::green(), None);
        // 第三段切回显式 SrcOver。
        canvas.set_blend_mode(BlendMode::SrcOver);
        // 触发 Additive 段封口并建立最后一个当前段。
        canvas.fill_rect(Rect::new(9.0, 1.0, 2.0, 2.0), Color::blue(), None);
        // 只读快照必须同时包含两个已封口段和当前段。
        let segments = canvas.packed_soft_segments();
        // blend 切换产生且只产生三个连续段。
        assert_eq!(segments.len(), 3);
        // 第一段使用普通 premultiplied SrcOver pipeline。
        assert!(!segments[0].additive);
        // 中间段保留 Additive pipeline 事实。
        assert!(segments[1].additive);
        // 最后一段恢复 SrcOver pipeline。
        assert!(!segments[2].additive);
        // 每个 2x2 紧密 tile 都只携带四个可见像素。
        assert!(segments.iter().all(|segment| segment.pixels.len() == 4));
        // 三段目标横坐标必须保持原始绘制顺序。
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.tile.dst_x)
                .collect::<Vec<_>>(),
            vec![1, 5, 9]
        );
    }

    // 成功提交必须消费全部 soft 分段，后续操作不能重复上传旧像素。
    #[test]
    fn soft_segment_commit_clears_staging_without_changing_public_blend() {
        // 默认能力让测试只观察 soft staging 生命周期。
        let mut canvas = NativeGpuCanvas2D::new(12, 8, NativeRasterCaps::default());
        // 建立一个普通 soft 段。
        canvas.fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::red(), None);
        // 切换并建立一个 Additive 当前段。
        canvas.set_blend_mode(BlendMode::Additive);
        // 写入第二段以形成可消费的两段队列。
        canvas.fill_rect(Rect::new(5.0, 1.0, 2.0, 2.0), Color::green(), None);
        // 提交前必须能观察到两个有序段。
        assert_eq!(canvas.packed_soft_segments().len(), 2);
        // 模拟 RHI 全部提交成功后的统一消费边界。
        canvas.commit_presented_frame();
        // 旧段不能在下一次提交快照中再次出现。
        assert!(canvas.packed_soft_segments().is_empty());
        // 全局和当前 soft 内容标记都必须归零。
        assert!(!canvas.soft_has_content && !canvas.soft_current_has_content);
        // 已封口队列必须释放共享像素载荷。
        assert!(canvas.pending_soft_segments.is_empty());
        // 内部分段类别等待下一次真实绘制重新建立。
        assert!(canvas.soft_segment_blend.is_none());
        // Canvas 的公开 blend 状态仍由调用方控制，不在内部提交时篡改。
        assert_eq!(canvas.current_blend_mode(), BlendMode::Additive);
        // 下一次绘制应建立一个全新的 Additive 段。
        canvas.fill_rect(Rect::new(8.0, 1.0, 2.0, 2.0), Color::blue(), None);
        // 新快照只包含提交后的新段。
        let segments = canvas.packed_soft_segments();
        // 不得重新带出此前的两个段。
        assert_eq!(segments.len(), 1);
        // 新段继承仍然有效的公开 Additive 状态。
        assert!(segments[0].additive);
        // 新段目标位置必须来自提交后的绘制。
        assert_eq!(segments[0].tile.dst_x, 8);
    }
}
