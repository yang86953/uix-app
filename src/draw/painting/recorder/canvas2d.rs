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
                self.encoder_mut()?
                    .native(FrameRasterOp::StrokeRoundedRects {
                        // 当前调用只产生一条描边，后续由 encoder 做安全批合并。
                        strokes: vec![FrameStrokeRect::new(
                            native_rect,
                            native_color,
                            native_radius,
                            line_width,
                        )],
                        // 保存已经证明为整数矩形的当前 clip。
                        clip,
                        // 标记该批必须使用饱和加法混合。
                        additive: true,
                    });
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
                self.encoder_mut()?
                    .native(FrameRasterOp::StrokeRoundedRects {
                        strokes: vec![FrameStrokeRect::new(
                            native_rect,
                            native_color,
                            native_radius,
                            line_width,
                        )],
                        clip,
                        // 普通直达描边保持 SrcOver 语义。
                        additive: false,
                    });
                Ok(())
            }) {
                self.remember_error(error);
            }
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

// 验证 Additive 描边从 Canvas2D 记录到参考像素的完整语义。
#[cfg(test)]
mod tests {
    // 引入当前 recorder 实现和 Canvas2D 依赖类型。
    use super::*;
    // 引入命令枚举以审计实际记录载荷。
    use crate::draw::painting::FrameCommand;

    // 矩形与圆形 Additive 描边应共享一个明确标记的安全批次。
    #[test]
    fn records_additive_stroke_and_reference_adds_destination() {
        // 创建能够容纳两个互不相交描边的 recorder 画布。
        let mut canvas = FrameRecordingCanvas::new(16, 8);
        // 开始一帧带透明 clear 的正式记录。
        if let Err(error) = canvas.begin_recording(true) {
            // 合法尺寸的记录初始化不得失败。
            panic!("additive stroke recording should begin: {error:?}");
        }
        // 先用不透明红色建立可观察的累计目标。
        canvas.fill_rect(Rect::new(0.0, 0.0, 16.0, 8.0), Color::red(), None);
        // 后续描边切换到目标相关 Additive 混合。
        canvas.set_blend_mode(BlendMode::Additive);
        // 记录一个整数轴对齐直角矩形描边。
        canvas.stroke_rect(Rect::new(1.0, 1.0, 4.0, 4.0), Color::green(), 1.0, None);
        // 记录一个与前一描边互不相交的整数轴对齐圆形描边。
        canvas.stroke_circle(12.0, 3.0, 2.0, Color::green(), 1.0);
        // 完成记录并取得不可变命令流。
        let encoder = match canvas.finish_recording() {
            // 保存成功的编码器供载荷和像素审计。
            Ok(encoder) => encoder,
            // 合法 Additive 描边不应产生 deferred failure。
            Err(error) => panic!("additive stroke recording should finish: {error:?}"),
        };
        // 命令流应严格为 clear、红色底和一个 Additive 描边批次。
        let [FrameCommand::Clear { .. }, FrameCommand::Native {
            operation: FrameRasterOp::FillRect { .. },
        }, FrameCommand::Native {
            operation:
                FrameRasterOp::StrokeRoundedRects {
                    strokes,
                    additive: true,
                    ..
                },
        }] = encoder.commands()
        else {
            // 任何额外 CPU segment 或拆错顺序都说明记录路径退化。
            panic!("expected clear, fill, and one additive stroke batch");
        };
        // 相同 clip/blend 且互不相交的矩形和圆应安全合为一批。
        assert_eq!(strokes.len(), 2);
        // 第二条圆形描边必须保留半径事实而不是退化为直角矩形。
        assert_eq!(strokes[1].radius().to_radius().tl, 2.0);
        // 执行 CPU 参考路径以核验真实目标相关混合。
        let reference = encoder.render_reference();
        // 红底上的绿色 Additive 描边应逐通道饱和为黄色。
        assert_eq!(
            reference.pixel(1, 1),
            Some(Color::from_rgb(255, 255, 0).premultiplied())
        );
        // 描边内部未覆盖像素必须继续保持原始红色目标。
        assert_eq!(reference.pixel(2, 2), Some(Color::red().premultiplied()));
    }

    // 合法 Additive 填充圆必须直接保留为圆角 shape，并对累计目标执行加法。
    #[test]
    fn records_additive_fill_circle_as_rounded_shape() {
        // 创建能够明确区分圆内外像素的录制画布。
        let mut canvas = FrameRecordingCanvas::new(8, 8);
        // 开始一帧带透明 clear 的正式记录。
        if let Err(error) = canvas.begin_recording(true) {
            // 合法尺寸的记录初始化不得失败。
            panic!("additive circle recording should begin: {error:?}");
        }
        // 先用不透明红色建立可观察的累计目标。
        canvas.fill_rect(Rect::new(0.0, 0.0, 8.0, 8.0), Color::red(), None);
        // 后续圆形填充切换到目标相关 Additive 混合。
        canvas.set_blend_mode(BlendMode::Additive);
        // 记录边界完全位于 surface 内的整数圆。
        canvas.fill_circle(4.0, 4.0, 2.0, Color::green());
        // 完成记录并取得不可变命令流。
        let encoder = match canvas.finish_recording() {
            // 保存成功的编码器供载荷和像素审计。
            Ok(encoder) => encoder,
            // 合法 Additive 圆不应产生 deferred failure。
            Err(error) => panic!("additive circle recording should finish: {error:?}"),
        };
        // 精确匹配命令序列，同时证明没有插入透明 CPU segment。
        let [FrameCommand::Clear { .. }, FrameCommand::Native {
            operation: FrameRasterOp::FillRect { .. },
        }, FrameCommand::Native {
            operation:
                FrameRasterOp::FillRoundedRectAdditive {
                    rect,
                    color,
                    radius,
                    clip,
                },
        }] = encoder.commands()
        else {
            // 任何 CPU segment 或普通 blend shape 都说明准入路径仍然错误。
            panic!("expected clear, fill, and one additive rounded circle");
        };
        // 圆应保留为以 (2, 2) 起始的 4×4 正方形。
        assert_eq!(*rect, FrameRect::new(2, 2, 4, 4));
        // Additive shape 必须保留调用方提供的绿色源色。
        assert_eq!(*color, Color::green());
        // 四角半径应等于原始圆半径，避免退化为直角矩形。
        assert_eq!(radius.to_radius(), Radius::uniform(2.0));
        // 未设置局部裁剪时载荷必须显式保存完整 surface。
        assert_eq!(*clip, FrameRect::new(0, 0, 8, 8));
        // 执行 CPU 参考路径以核验真实目标相关混合。
        let reference = encoder.render_reference();
        // 红底圆心叠加绿色后应逐通道饱和为黄色。
        assert_eq!(
            reference.pixel(4, 4),
            Some(Color::from_rgb(255, 255, 0).premultiplied())
        );
        // 圆外像素不得受 Additive shape 影响。
        assert_eq!(reference.pixel(0, 0), Some(Color::red().premultiplied()));
    }

    // Additive 填充与描边必须共用当前局部矩形裁剪，空裁剪则保持 no-op。
    #[test]
    fn additive_shapes_preserve_partial_rect_clip() {
        // 创建能够跨越裁剪边界并保留未裁剪目标的录制画布。
        let mut canvas = FrameRecordingCanvas::new(12, 10);
        // 开始一帧带透明 clear 的正式记录。
        if let Err(error) = canvas.begin_recording(true) {
            // 合法尺寸的记录初始化不得失败。
            panic!("clipped additive recording should begin: {error:?}");
        }
        // 先用不透明红色建立可观察的累计目标。
        canvas.fill_rect(Rect::new(0.0, 0.0, 12.0, 10.0), Color::red(), None);
        // 将后续操作限制在左半侧整数矩形内。
        canvas.push_clip(Rect::new(0.0, 0.0, 6.0, 10.0));
        // 后续填充与描边切换到目标相关 Additive 混合。
        canvas.set_blend_mode(BlendMode::Additive);
        // 记录一个横跨 x=6 裁剪边界的圆形填充。
        canvas.fill_circle(6.0, 3.0, 2.0, Color::green());
        // 记录一个同样横跨裁剪边界的直角矩形描边。
        canvas.stroke_rect(Rect::new(4.0, 6.0, 4.0, 3.0), Color::green(), 1.0, None);
        // 追加与当前裁剪完全不相交的子裁剪以形成空裁剪。
        canvas.push_clip(Rect::new(20.0, 0.0, 2.0, 2.0));
        // 空裁剪下的合法 Additive 矩形必须成为 no-op。
        canvas.fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::blue(), None);
        // 完成记录并取得不可变命令流。
        let encoder = match canvas.finish_recording() {
            // 保存成功的编码器供载荷和像素审计。
            Ok(encoder) => encoder,
            // 合法局部裁剪不应产生 deferred failure。
            Err(error) => panic!("clipped additive recording should finish: {error:?}"),
        };
        // 精确匹配命令序列，同时证明空裁剪没有追加第五条命令。
        let [FrameCommand::Clear { .. }, FrameCommand::Native {
            operation: FrameRasterOp::FillRect { .. },
        }, FrameCommand::Native {
            operation:
                FrameRasterOp::FillRoundedRectAdditive {
                    clip: fill_clip, ..
                },
        }, FrameCommand::Native {
            operation:
                FrameRasterOp::StrokeRoundedRects {
                    strokes,
                    clip: stroke_clip,
                    additive: true,
                },
        }] = encoder.commands()
        else {
            // CPU segment、错误批次或空裁剪命令都会破坏这一精确事实。
            panic!("expected clipped additive fill and stroke commands");
        };
        // 填充载荷必须保存左半侧逻辑裁剪。
        assert_eq!(*fill_clip, FrameRect::new(0, 0, 6, 10));
        // 描边批次必须保存与填充完全相同的逻辑裁剪。
        assert_eq!(*stroke_clip, FrameRect::new(0, 0, 6, 10));
        // 当前调用只应产生一条裁剪描边。
        assert_eq!(strokes.len(), 1);
        // 执行 CPU 参考路径以核验裁剪前后的目标相关像素。
        let reference = encoder.render_reference();
        // 裁剪内的圆心左侧像素应由红绿相加得到黄色。
        assert_eq!(
            reference.pixel(5, 3),
            Some(Color::from_rgb(255, 255, 0).premultiplied())
        );
        // 裁剪外的相邻圆内像素必须继续保持原始红色目标。
        assert_eq!(reference.pixel(6, 3), Some(Color::red().premultiplied()));
    }

    // 有限整数 offset 必须平移 Additive 几何，同时保持 surface-space clip 不变。
    #[test]
    fn additive_shapes_map_integral_offsets_without_moving_clip_twice() {
        // 创建能够容纳正负平移后几何的录制画布。
        let mut canvas = FrameRecordingCanvas::new(14, 10);
        // 开始一帧带透明 clear 的正式记录。
        if let Err(error) = canvas.begin_recording(true) {
            // 合法尺寸的记录初始化不得失败。
            panic!("offset additive recording should begin: {error:?}");
        }
        // 先用不透明红色建立可观察的累计目标。
        canvas.fill_rect(Rect::new(0.0, 0.0, 14.0, 10.0), Color::red(), None);
        // 后续几何先应用正整数像素 offset。
        canvas.set_offset(2.0, 1.0);
        // 局部 clip 也在当前状态下映射一次到 surface 坐标。
        canvas.push_clip(Rect::new(1.0, 1.0, 3.0, 4.0));
        // 后续填充与描边切换到目标相关 Additive 混合。
        canvas.set_blend_mode(BlendMode::Additive);
        // 本地圆形边界 (1,1,4,4) 应平移为 surface 矩形 (3,2,4,4)。
        canvas.fill_circle(3.0, 3.0, 2.0, Color::green());
        // 恢复完整 surface clip，避免后一命令与前一命令共享裁剪。
        canvas.pop_clip();
        // 切换到负整数 offset 以覆盖反方向映射。
        canvas.set_offset(-2.0, -1.0);
        // 本地矩形 (4,6,4,3) 应平移为 surface 矩形 (2,5,4,3)。
        canvas.stroke_rect(Rect::new(4.0, 6.0, 4.0, 3.0), Color::green(), 1.0, None);
        // 完成记录并取得不可变命令流。
        let encoder = match canvas.finish_recording() {
            // 保存成功的编码器供载荷和像素审计。
            Ok(encoder) => encoder,
            // 合法整数 offset 不应产生 deferred failure。
            Err(error) => panic!("offset additive recording should finish: {error:?}"),
        };
        // 精确匹配命令序列，同时证明两条 Additive 操作都没有进入 CPU segment。
        let [FrameCommand::Clear { .. }, FrameCommand::Native {
            operation: FrameRasterOp::FillRect { .. },
        }, FrameCommand::Native {
            operation:
                FrameRasterOp::FillRoundedRectAdditive {
                    rect: fill_rect,
                    clip: fill_clip,
                    ..
                },
        }, FrameCommand::Native {
            operation:
                FrameRasterOp::StrokeRoundedRects {
                    strokes,
                    clip: stroke_clip,
                    additive: true,
                },
        }] = encoder.commands()
        else {
            // CPU segment、错误几何或错误批次都会破坏这一精确事实。
            panic!("expected offset additive fill and stroke commands");
        };
        // 正 offset 必须只平移圆的正方形几何一次。
        assert_eq!(*fill_rect, FrameRect::new(3, 2, 4, 4));
        // 当前 clip 已是 surface 坐标，必须保持 (3,2,3,4) 而不能再次平移。
        assert_eq!(*fill_clip, FrameRect::new(3, 2, 3, 4));
        // 负 offset 下当前调用只应产生一条描边。
        assert_eq!(strokes.len(), 1);
        // 负 offset 必须把本地描边矩形平移到 surface 左上侧。
        assert_eq!(strokes[0].rect(), FrameRect::new(2, 5, 4, 3));
        // pop_clip 后的描边必须恢复完整 surface 裁剪。
        assert_eq!(*stroke_clip, FrameRect::new(0, 0, 14, 10));
        // 执行 CPU 参考路径以核验平移和裁剪后的真实目标像素。
        let reference = encoder.render_reference();
        // 正 offset 后裁剪内的圆形像素应由红绿相加得到黄色。
        assert_eq!(
            reference.pixel(5, 3),
            Some(Color::from_rgb(255, 255, 0).premultiplied())
        );
        // x=6 位于圆内但在 surface-space clip 外，必须保持原始红色。
        assert_eq!(reference.pixel(6, 3), Some(Color::red().premultiplied()));
        // 负 offset 后的描边左上像素也应对红色目标执行加法。
        assert_eq!(
            reference.pixel(2, 5),
            Some(Color::from_rgb(255, 255, 0).premultiplied())
        );
    }

    // 分数 offset 无法进入固定 Native shape 时应保留在 Additive sampled soft 分段中。
    #[test]
    fn additive_fill_preserves_fractional_offset_in_sampled_segment() {
        // 创建一个最小但足以容纳测试矩形的录制画布。
        let mut canvas = FrameRecordingCanvas::new(8, 8);
        // 开始一帧带透明 clear 的正式记录。
        if let Err(error) = canvas.begin_recording(true) {
            // 合法尺寸的记录初始化不得失败。
            panic!("fractional offset recording should begin: {error:?}");
        }
        // 设置不能无损映射为 FrameRect 的水平分数 offset。
        canvas.set_offset(0.5, 0.0);
        // 选择目标相关 Additive 混合。
        canvas.set_blend_mode(BlendMode::Additive);
        // 记录一个 otherwise 合法的整数矩形。
        canvas.fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::green(), None);
        // 完成记录并取得用于审计命令和像素的编码器。
        let encoder = match canvas.finish_recording() {
            // 合法分数 offset 应由软件光栅保真处理。
            Ok(encoder) => encoder,
            // typed failure 表示新 fallback 没有覆盖该几何。
            Err(error) => panic!("fractional additive offset should finish: {error:?}"),
        };
        // 分数几何不能伪装成整数 Native shape 或 SrcOver CPU segment。
        assert!(matches!(
            encoder.commands(),
            [
                FrameCommand::Clear { .. },
                FrameCommand::PictureBlit { additive: true, .. }
            ]
        ));
        // 矩形内部的完全覆盖像素应保留原始绿色源贡献。
        assert_eq!(
            encoder.render_reference().pixel(2, 2),
            Some(Color::green().premultiplied())
        );
    }

    // 继续在同一测试模块内加载纯平移 transform 的独立回归测试。
    include!("canvas2d_test_tail.rs");

    // 继续在同一测试模块内加载 Additive sampled soft 分段回归测试。
    include!("canvas2d_additive_soft_tests.rs");

    // 继续加载 Additive 仿射描边 sampled soft 分段回归测试。
    include!("canvas2d_additive_stroke_soft_tests.rs");

    // 继续在同一测试模块内加载 Additive opacity 的独立回归测试。
    include!("canvas2d_opacity_tests.rs");
}
