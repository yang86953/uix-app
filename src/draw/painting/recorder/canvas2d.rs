//! [`FrameRecordingCanvas`] 的 `Canvas2D` 实现 — recorder 子模块。
//!
//! 每个绘制原语选择 native / scratch / direct 三条路径之一；仅当语义等价
//! 证明成立才允许 native 或 direct 下放。

use crate::core::Rect;
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::types::{BlendMode, GradientDirection, Radius, Transform};
use crate::draw::painting::{
    FrameRadius, FrameRasterOp, FrameRect, FrameStrokeRect, FrameStrokeWidth,
};
use crate::draw::{Canvas2D, Color};
use std::sync::Arc;

use super::canvas::FrameRecordingCanvas;
use super::geometry::{frame_encoder_error, rect_to_frame};

impl Canvas2D for FrameRecordingCanvas {
    /// 读取当前仿射变换。
    fn current_transform(&self) -> Transform {
        self.scratch.current_transform()
    }

    /// 设置当前仿射变换。
    fn set_transform(&mut self, transform: Transform) {
        self.scratch.set_transform(transform);
    }

    /// 读取像素级平移 offset。
    fn offset(&self) -> (f32, f32) {
        self.scratch.offset()
    }

    /// 设置像素级平移 offset。
    fn set_offset(&mut self, dx: f32, dy: f32) {
        self.scratch.set_offset(dx, dy);
    }

    /// 追加像素级平移。
    fn translate(&mut self, dx: f32, dy: f32) {
        self.scratch.translate(dx, dy);
    }

    /// 录制矩形填充：Additive 走 Native shape，SrcOver 可直达则直达，否则软回退。
    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        // Additive 提升同时取得已经验证的几何与矩形裁剪事实。
        if let Some((rect, clip, native_color, radius)) =
            self.native_additive_shape(rect, color, radius)
        {
            // 完全被裁掉的操作是安全 no-op，不需要 flush 或命令载荷。
            if clip.is_empty() {
                // 保持当前命令流不变。
                return;
            }
            // opacity 或源 alpha 量化为全透明时同样是安全 no-op。
            if native_color.a == 0 {
                // 不 flush 既有 scratch，也不追加无贡献 Native 命令。
                return;
            }
            // 非透明操作才需要验证可编码的圆角载荷。
            let additive_radius = match radius.map(FrameRadius::new).transpose() {
                // 保存合法的可选圆角。
                Ok(radius) => radius,
                // 非法圆角继续转换为既有 deferred typed failure。
                Err(error) => {
                    // 记录统一的 encoder 错误。
                    self.remember_error(frame_encoder_error(error));
                    // 非法操作不能继续记录。
                    return;
                }
            };
            if let Err(error) = self.flush_scratch().and_then(|()| {
                let operation = match additive_radius {
                    Some(radius) => FrameRasterOp::FillRoundedRectAdditive {
                        rect,
                        color: native_color,
                        radius,
                        // 保存当前整数矩形裁剪，供参考执行与 RHI scissor 共用。
                        clip,
                    },
                    None => FrameRasterOp::FillRectAdditive {
                        rect,
                        color: native_color,
                        // 普通矩形也必须保留同一个 Additive 裁剪事实。
                        clip,
                    },
                };
                self.encoder_mut()?.native(operation);
                Ok(())
            }) {
                self.remember_error(error);
            }
            return;
        }
        if let Some((native_rect, clip)) = self.native_src_over_rects(rect) {
            let native_color = crate::draw::raster::rasterizer::color_with_premultiplied_opacity(
                color,
                self.scratch.opacity(),
            );
            let native_radius = match radius.map(FrameRadius::new).transpose() {
                Ok(radius) => radius,
                Err(_) => {
                    // 普通 blend 的非法半径历史上由 CPU rasterizer 处理；
                    // 这里只拒绝提升，不把既有 void API 改成 deferred typed failure。
                    self.draw_cpu(rect, 1.0, |scratch| scratch.fill_rect(rect, color, radius));
                    return;
                }
            };
            if clip.width <= 0 || clip.height <= 0 {
                return;
            }
            if let Err(error) = self.flush_scratch().and_then(|()| {
                let full_clip = FrameRect::new(0, 0, self.width, self.height);
                let operation = if clip == full_clip {
                    match native_radius {
                        Some(radius) => {
                            let value = radius.to_radius();
                            if value.tl != 0.0
                                || value.tr != 0.0
                                || value.br != 0.0
                                || value.bl != 0.0
                            {
                                FrameRasterOp::FillRoundedRect {
                                    rect: native_rect,
                                    color: native_color,
                                    radius,
                                }
                            } else {
                                FrameRasterOp::FillRect {
                                    rect: native_rect,
                                    color: native_color,
                                }
                            }
                        }
                        None => FrameRasterOp::FillRect {
                            rect: native_rect,
                            color: native_color,
                        },
                    }
                } else {
                    FrameRasterOp::FillRoundedRectClipped {
                        rect: native_rect,
                        color: native_color,
                        radius: native_radius.unwrap_or_else(FrameRadius::zero),
                        clip,
                    }
                };
                self.encoder_mut()?.native(operation);
                Ok(())
            }) {
                self.remember_error(error);
            }
            return;
        }
        // 其余 identity 亚像素矩形优先记录为普通 SrcOver GPU shape。
        if self.record_src_over_subpixel_fill(rect, color, radius) {
            // 原生命令已经完整保存当前几何与透明度语义。
            return;
        }
        // Native shape 不适用时，Additive 填充改走目标相关 sampled soft segment。
        self.draw_cpu_source(rect, 1.0, |scratch| {
            // 由共享软件光栅保留完整 transform、clip 与圆角语义。
            scratch.fill_rect(rect, color, radius)
        });
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        // 把圆形转换为共享 rounded-rect SDF 所需的正方形边界。
        let bounds = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        // SrcOver 与已证明安全的 Additive 圆都复用 fill_rect 的统一命令记录路径。
        if r.is_finite()
            && r > 0.0
            && (self
                .native_additive_shape(bounds, color, Some(Radius::uniform(r)))
                .is_some()
                || self.native_src_over_rects(bounds).is_some())
        {
            // 正方形四角半径等于圆半径时，与目标圆的共享 SDF 完全一致。
            self.fill_rect(bounds, color, Some(Radius::uniform(r)));
            // 命令已经直接记录，禁止再生成 CPU segment。
            return;
        }
        // 其余圆形交给可按 blend 分段的软件填充路径。
        self.draw_cpu_source(bounds, 1.0, |scratch| {
            // Additive 源贡献会在透明 scratch 中光栅化。
            scratch.fill_circle(cx, cy, r, color)
        });
    }

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        // 椭圆没有固定 FrameRasterOp，由 sampled soft segment 承接 Additive。
        self.draw_cpu_source(rect, 1.0, |scratch| {
            // 软件椭圆入口负责完整仿射逆映射。
            scratch.fill_ellipse(rect, color)
        });
    }

    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, sa: f32, ea: f32, color: Color) {
        let bounds = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        // 扇形与其他填充共享 blend-aware scratch 分段。
        self.draw_cpu_source(bounds, 1.0, |scratch| {
            // 保留角度、transform 与局部裁剪语义。
            scratch.fill_sector(cx, cy, r, sa, ea, color)
        });
    }

    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        let bounds = path
            .bounds()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, self.width as f32, self.height as f32));
        // 路径填充在软件端完成仿射几何后进入同一 sampled 分段。
        self.draw_cpu_source(bounds, 1.0, |scratch| {
            // 保留调用方选择的填充规则。
            scratch.fill_path(path, color, fill_rule)
        });
    }

    /// 录制矩形描边：Additive 与可直达 SrcOver 走 Native，其余软回退。
    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32, radius: Option<Radius>) {
        // 已证明安全的固定 Additive 描边优先保留为目标相关 Native 命令。
        if let Some((native_rect, clip, native_color, radius)) =
            self.native_additive_shape(rect, color, radius)
        {
            // 完全不可见的描边不产生命令，也不需要触碰累计目标。
            if clip.is_empty() {
                // 保持既有命令流和 staging 不变。
                return;
            }
            // opacity 或源 alpha 量化为全透明时不应触碰累计目标。
            if native_color.a == 0 {
                // 透明 Additive 描边既不 flush，也不产生命令。
                return;
            }
            // 在记录边界验证圆角，避免把非法浮点几何带入 FrameEncoder。
            let native_radius = match FrameRadius::new(radius.unwrap_or_default()) {
                // 保存通过验证的圆角值。
                Ok(radius) => radius,
                // 将非法圆角转为既有 deferred typed failure。
                Err(error) => {
                    // 记录统一的 encoder 错误。
                    self.remember_error(frame_encoder_error(error));
                    // 非法操作不能继续记录。
                    return;
                }
            };
            // 在记录边界验证正有限描边宽度。
            let line_width = match FrameStrokeWidth::new(width) {
                // 保存通过验证的描边宽度。
                Ok(width) => width,
                // 将非法宽度转为既有 deferred typed failure。
                Err(error) => {
                    // 记录统一的 encoder 错误。
                    self.remember_error(frame_encoder_error(error));
                    // 非法操作不能继续记录。
                    return;
                }
            };
            // 目标相关命令前必须先提交此前累计的 source-independent scratch。
            if let Err(error) = self.flush_scratch().and_then(|()| {
                // 记录带显式 blend 事实的共享描边载荷。
                self.encoder_mut()?.native_stroke(
                    FrameStrokeRect::new(native_rect, native_color, native_radius, line_width),
                    // 保存已经证明为整数矩形的当前 clip。
                    clip,
                    // 标记该批必须使用饱和加法混合。
                    true,
                );
                // 命令记录成功。
                Ok(())
            }) {
                // 延迟报告 flush 或 encoder 状态错误。
                self.remember_error(error);
            }
            // Additive 路径已经完整处理。
            return;
        }
        if let Some((native_rect, clip)) = self.native_src_over_rects(rect) {
            let native_radius = radius.unwrap_or_default();
            let (Ok(native_radius), Ok(line_width)) = (
                FrameRadius::new(native_radius),
                FrameStrokeWidth::new(width),
            ) else {
                self.draw_cpu(rect, width.max(1.0), |scratch| {
                    scratch.stroke_rect(rect, color, width, radius)
                });
                return;
            };
            if clip.width <= 0 || clip.height <= 0 {
                return;
            }
            let native_color = crate::draw::raster::rasterizer::color_with_premultiplied_opacity(
                color,
                self.scratch.opacity(),
            );
            if native_color.a == 0 {
                return;
            }
            if let Err(error) = self.flush_scratch().and_then(|()| {
                self.encoder_mut()?.native_stroke(
                    FrameStrokeRect::new(native_rect, native_color, native_radius, line_width),
                    clip,
                    // 普通直达描边保持 SrcOver 语义。
                    false,
                );
                Ok(())
            }) {
                self.remember_error(error);
            }
            return;
        }
        // 其余 identity 亚像素矩形优先记录为普通 SrcOver GPU 描边。
        if self.record_src_over_subpixel_stroke(rect, color, width, radius) {
            // 原生命令已经完整保存当前描边语义。
            return;
        }
        // 固定 Native shape 不适用时，描边作为纯源贡献进入 blend-aware scratch。
        self.draw_cpu_source(rect, width.max(1.0), |scratch| {
            // 软件描边负责完整 transform、clip、opacity 与圆角语义。
            scratch.stroke_rect(rect, color, width, radius)
        });
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, width: f32) {
        let bounds = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        if r.is_finite()
            && r > 0.0
            && (self
                .native_additive_shape(bounds, color, Some(Radius::uniform(r)))
                .is_some()
                || self.native_src_over_rects(bounds).is_some())
        {
            // 圆形描边是共享 rounded-rect 描边 SDF 在「四角半径等于边长
            // 一半的正方形」上的特例。
            self.stroke_rect(bounds, color, width, Some(Radius::uniform(r)));
            return;
        }
        // 非固定圆形描边走与填充共享的 Additive sampled scratch。
        self.draw_cpu_source(bounds, width.max(1.0), |scratch| {
            // 软件圆形描边在本地空间保持线宽后再执行仿射映射。
            scratch.stroke_circle(cx, cy, r, color, width)
        });
    }

    fn stroke_path(&mut self, path: &Path, color: Color, options: &StrokeOptions) {
        let bounds = path
            .bounds()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, self.width as f32, self.height as f32));
        // 路径描边轮廓是可结合的源贡献，可安全进入 Additive sampled segment。
        self.draw_cpu_source(
            // 路径控制点边界作为本地基准。
            bounds,
            // miter 可能扩展到线宽倍数，先保守扩大扫描范围。
            options.width.max(1.0) * options.miter_limit.max(1.0),
            // 在透明 scratch 中生成完整描边轮廓。
            |scratch| {
                // 软件 stroker 保留 cap、join、miter 与完整仿射语义。
                scratch.stroke_path(path, color, options)
            },
        );
    }
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        let bounds = Rect::new(
            x1.min(x2),
            y1.min(y2),
            (x1 - x2).abs().max(1.0),
            (y1 - y2).abs().max(1.0),
        );
        // 直线描边同样使用 blend-aware scratch，避免 Additive 被错误拒绝。
        self.draw_cpu_source(bounds, width.max(1.0), |scratch| {
            // 软件直线入口负责本地线宽与仿射映射。
            scratch.draw_line(x1, y1, x2, y2, color, width)
        });
    }
    fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
    ) {
        // 渐变是可结合的纯源贡献，可安全进入 Additive sampled scratch。
        self.draw_cpu_source(rect, 1.0, |scratch| {
            // 软件渐变负责 offset 后的完整仿射、裁剪与 opacity。
            scratch.fill_linear_gradient(rect, color_a, color_b, dir)
        });
    }
    fn fill_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        inner_r: f32,
        outer_r: f32,
        inner_color: Color,
        outer_color: Color,
    ) {
        let bounds = Rect::new(cx - outer_r, cy - outer_r, outer_r * 2.0, outer_r * 2.0);
        // 径向渐变同样只生成源贡献，并与相邻填充/描边共享 Additive 批次。
        self.draw_cpu_source(bounds, 1.0, |scratch| {
            // 局部圆经过任意仿射后由 inverse sampling 保持渐变定义。
            scratch.fill_radial_gradient(cx, cy, inner_r, outer_r, inner_color, outer_color)
        });
    }
    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        radius: Option<Radius>,
    ) {
        let pad = blur.max(0.0) + offset_x.abs().max(offset_y.abs()) + 1.0;
        // 阴影是可结合的纯源贡献，可安全进入 Additive sampled scratch。
        self.draw_cpu_source(rect, pad, |scratch| {
            scratch.draw_box_shadow(rect, blur, offset_x, offset_y, color, radius)
        });
    }

    fn draw_box_shadow_ambient(
        &mut self,
        rect: Rect,
        blur: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        radius: Option<Radius>,
    ) {
        let pad = blur.max(0.0) + offset_x.abs().max(offset_y.abs()) + 1.0;
        // 环境阴影与定向阴影共享相同的 Additive source scratch 边界。
        self.draw_cpu_source(rect, pad, |scratch| {
            scratch.draw_box_shadow_ambient(rect, blur, offset_x, offset_y, color, radius)
        });
    }
    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        if self.deferred_error.is_some() {
            return;
        }
        match self.record_direct_image_blit(src, src_w, src_rect, dst_rect) {
            Ok(true) => return,
            Ok(false) => {}
            Err(error) => {
                self.remember_error(error);
                return;
            }
        }
        // 不能直达的图片仍是可结合源贡献，可进入 Additive sampled scratch。
        self.draw_cpu_source(dst_rect, 1.0, |scratch| {
            // 共享软件图片负责 offset 后仿射、clip、opacity 与 nearest sampling。
            scratch.blit_image(src, src_w, src_rect, dst_rect)
        });
    }

    fn blit_image_shared(
        &mut self,
        src: std::sync::Arc<Vec<u32>>,
        src_w: i32,
        src_rect: Rect,
        dst_rect: Rect,
    ) {
        if self.deferred_error.is_some() {
            return;
        }
        match self.record_direct_image_blit_shared(
            std::sync::Arc::clone(&src),
            src_w,
            src_rect,
            dst_rect,
        ) {
            Ok(true) => return,
            Ok(false) => {}
            Err(error) => {
                self.remember_error(error);
                return;
            }
        }
        // 不能直达时在本次同步调用内借用共享像素，结束后不保留裸切片。
        self.draw_cpu_source(dst_rect, 1.0, |scratch| {
            scratch.blit_image(src.as_slice(), src_w, src_rect, dst_rect)
        });
    }

    /// 录制字形（带 coverage 借用切片）。
    fn blit_glyph(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        width: usize,
        height: usize,
        color: Color,
    ) {
        self.record_glyph_shared(x, y, Arc::from(coverage), width, height, color);
    }

    /// 录制字形（共享 coverage，避免拷贝）。
    fn blit_glyph_shared(
        &mut self,
        x: i32,
        y: i32,
        coverage: Arc<[u8]>,
        width: usize,
        height: usize,
        color: Color,
    ) {
        self.record_glyph_shared(x, y, coverage, width, height, color);
    }

    /// 优先录制可由 GPU MSDF 执行的共享字形轮廓。
    fn blit_glyph_outline_shared(
        &mut self,
        x: i32,
        y: i32,
        mesh: Arc<[f32]>,
        area_coverage: Option<Arc<[u8]>>,
        width: usize,
        height: usize,
        color: Color,
    ) {
        // identity SrcOver 字形优先保留轮廓，由 GPU 端生成 MSDF coverage。
        if self.record_glyph_outline_shared(x, y, Arc::clone(&mesh), width, height, color) {
            // 原生命令已经完整记录。
            return;
        }
        // 非原生状态优先复用字体服务提供的面积 coverage。
        let coverage = area_coverage.or_else(|| {
            // 缺少 coverage 时从同一轮廓生成软件兼容载荷。
            crate::draw::resources::font::glyph_outline::coverage_from_edges(
                mesh.as_ref(),
                width,
                height,
            )
            // 转为共享切片以复用普通字形记录路径。
            .map(Arc::<[u8]>::from)
        });
        // 只有成功取得合法 coverage 时才追加兼容命令。
        if let Some(coverage) = coverage {
            // 复用现有 coverage 准入、裁剪与软件 fallback 契约。
            self.record_glyph_shared(x, y, coverage, width, height, color);
        }
    }

    /// 保存画布状态（含 blend 镜像）。
    fn save(&mut self) {
        self.scratch.save();
        self.blend_stack.push(self.blend_mode);
    }

    fn restore(&mut self) {
        // 先取得即将恢复的 blend，用于判断 painter-order barrier。
        let restored_blend = self.blend_stack.last().copied();
        // 不同 blend 的透明源贡献不能共用一个最终合成命令。
        if restored_blend.is_some_and(|mode| mode != self.blend_mode) {
            // 在恢复 scratch 状态前提交当前批次。
            if let Err(error) = self.flush_scratch() {
                // 延迟报告 flush 失败。
                self.remember_error(error);
            }
        }
        // 恢复 transform、clip、opacity 与软件 blend 状态。
        self.scratch.restore();
        // 恢复录制器持有的 blend 镜像。
        if let Some(mode) = self.blend_stack.pop() {
            // 保存恢复后的 blend 事实。
            self.blend_mode = mode;
        }
    }

    /// 压入矩形裁剪。
    fn push_clip(&mut self, rect: Rect) {
        self.scratch.push_clip(rect);
    }

    /// 弹出最近一次裁剪。
    fn pop_clip(&mut self) {
        self.scratch.pop_clip();
    }

    /// 设置全局透明度。
    fn set_opacity(&mut self, opacity: f32) {
        self.scratch.set_opacity(opacity);
    }

    /// 读取当前全局透明度。
    fn opacity(&self) -> f32 {
        self.scratch.opacity()
    }

    fn set_blend_mode(&mut self, mode: BlendMode) {
        // blend 改变前必须封口当前透明 scratch，保持 painter order。
        if mode != self.blend_mode {
            // 将当前批次编码成其原始 blend 对应的命令。
            if let Err(error) = self.flush_scratch() {
                // 延迟报告 flush 或编码失败。
                self.remember_error(error);
            }
        }
        // 更新软件光栅 blend 状态。
        self.scratch.set_blend_mode(mode);
        // 更新录制器用于命令选择的 blend 镜像。
        self.blend_mode = mode;
    }

    /// 录制路径裁剪（仅 Additive 下可保真）。
    fn push_clip_path(&mut self, path: &Path) {
        self.record_path_clip(path);
    }

    /// 读取当前 scratch 像素（测试 / 诊断）。
    fn pixels(&self) -> &[u32] {
        self.scratch.surface().pixels()
    }

    /// 测试用可变像素入口（标记批次并沿用当前 blend 事实）。
    #[cfg(test)]
    fn pixels_mut(&mut self) -> &mut [u32] {
        if let Err(error) = self.ensure_scratch() {
            self.remember_error(error);
            return self.scratch.pixels_mut();
        }
        self.scratch_dirty = true;
        // 测试直接写像素时沿用当前 blend 的最终合成事实。
        self.scratch_additive = self.blend_mode == BlendMode::Additive;
        self.scratch.pixels_mut()
    }

    /// 读取 surface 尺寸。
    fn surface_size(&self) -> crate::core::Size {
        crate::core::Size::new(self.width as f32, self.height as f32)
    }

    /// 读取当前矩形裁剪。
    fn current_clip(&self) -> Rect {
        self.scratch.current_clip()
    }

    fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32) {
        if self.deferred_error.is_some() {
            return;
        }
        let int_dx = dx.round() as i32;
        let int_dy = dy.round() as i32;
        if int_dx == 0 && int_dy == 0 {
            return;
        }
        let Ok(frame_viewport) = rect_to_frame(viewport) else {
            self.unsupported_state("scroll-region with non-integral viewport");
            return;
        };
        if let Err(error) = self.flush_scratch().and_then(|()| {
            self.encoder_mut()?.native(FrameRasterOp::ScrollCopy {
                viewport: frame_viewport,
                dx: int_dx,
                dy: int_dy,
            });
            Ok(())
        }) {
            self.remember_error(error);
        }
    }
}

// 验证 Additive 形状从 Canvas2D 记录到参考像素的完整语义（外置测试文件）。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/draw/painting/recorder/canvas2d__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
