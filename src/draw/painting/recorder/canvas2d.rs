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
    fn current_transform(&self) -> Transform {
        self.scratch.current_transform()
    }

    fn set_transform(&mut self, transform: Transform) {
        self.scratch.set_transform(transform);
    }

    fn offset(&self) -> (f32, f32) {
        self.scratch.offset()
    }

    fn set_offset(&mut self, dx: f32, dy: f32) {
        self.scratch.set_offset(dx, dy);
    }

    fn translate(&mut self, dx: f32, dy: f32) {
        self.scratch.translate(dx, dy);
    }

    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        // Additive 提升同时取得已经验证的几何与矩形裁剪事实。
        if let Some((rect, clip, native_color)) = self.native_additive_shape(rect, color, radius) {
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
        self.draw_cpu(rect, 1.0, |scratch| scratch.fill_rect(rect, color, radius));
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
        // 其余普通 blend 保持软件光栅；Additive 会沿既有边界记录 typed failure。
        self.draw_cpu(bounds, 1.0, |scratch| scratch.fill_circle(cx, cy, r, color));
    }

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        self.draw_cpu(rect, 1.0, |scratch| scratch.fill_ellipse(rect, color));
    }

    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, sa: f32, ea: f32, color: Color) {
        let bounds = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        self.draw_cpu(bounds, 1.0, |scratch| {
            scratch.fill_sector(cx, cy, r, sa, ea, color)
        });
    }

    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        let bounds = path
            .bounds()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, self.width as f32, self.height as f32));
        self.draw_cpu(bounds, 1.0, |scratch| {
            scratch.fill_path(path, color, fill_rule)
        });
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32, radius: Option<Radius>) {
        // Additive 描边只能作为目标相关 Native 命令保留，禁止进入透明 CPU segment。
        if let Some((native_rect, clip, native_color)) =
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
        self.draw_cpu(rect, width.max(1.0), |scratch| {
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
            // A circle stroke is the shared rounded-rect stroke SDF over a
            // square whose four radii equal half the extent.
            self.stroke_rect(bounds, color, width, Some(Radius::uniform(r)));
            return;
        }
        self.draw_cpu(bounds, width.max(1.0), |scratch| {
            scratch.stroke_circle(cx, cy, r, color, width)
        });
    }

    fn stroke_path(&mut self, path: &Path, color: Color, options: &StrokeOptions) {
        let bounds = path
            .bounds()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, self.width as f32, self.height as f32));
        self.draw_cpu(bounds, options.width.max(1.0), |scratch| {
            scratch.stroke_path(path, color, options)
        });
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        let bounds = Rect::new(
            x1.min(x2),
            y1.min(y2),
            (x1 - x2).abs().max(1.0),
            (y1 - y2).abs().max(1.0),
        );
        self.draw_cpu(bounds, width.max(1.0), |scratch| {
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
        self.draw_cpu(rect, 1.0, |scratch| {
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
        self.draw_cpu(bounds, 1.0, |scratch| {
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
        self.draw_cpu(rect, pad, |scratch| {
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
        self.draw_cpu(rect, pad, |scratch| {
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
        self.draw_cpu(dst_rect, 1.0, |scratch| {
            scratch.blit_image(src, src_w, src_rect, dst_rect)
        });
    }

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

    fn save(&mut self) {
        self.scratch.save();
        self.blend_stack.push(self.blend_mode);
    }

    fn restore(&mut self) {
        self.scratch.restore();
        if let Some(mode) = self.blend_stack.pop() {
            self.blend_mode = mode;
        }
    }

    fn push_clip(&mut self, rect: Rect) {
        self.scratch.push_clip(rect);
    }

    fn pop_clip(&mut self) {
        self.scratch.pop_clip();
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.scratch.set_opacity(opacity);
    }

    fn opacity(&self) -> f32 {
        self.scratch.opacity()
    }

    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.scratch.set_blend_mode(mode);
        self.blend_mode = mode;
    }

    fn push_clip_path(&mut self, _path: &Path) {
        self.unsupported_state("path clip");
    }

    fn pixels(&self) -> &[u32] {
        self.scratch.surface().pixels()
    }

    #[cfg(test)]
    fn pixels_mut(&mut self) -> &mut [u32] {
        if let Err(error) = self.ensure_scratch() {
            self.remember_error(error);
            return self.scratch.pixels_mut();
        }
        self.scratch_dirty = true;
        self.scratch.pixels_mut()
    }

    fn surface_size(&self) -> crate::core::Size {
        crate::core::Size::new(self.width as f32, self.height as f32)
    }

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

    // 分数 offset 不能伪装成整数 FrameEncoder shape，必须保留 typed failure。
    #[test]
    fn additive_shape_rejects_fractional_offset() {
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
        // 尝试记录 otherwise 合法的整数矩形。
        canvas.fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::green(), None);
        // 完成边界必须返回稳定的 NotImplemented typed failure。
        let error = match canvas.finish_recording() {
            // 错误结果就是本测试需要审计的门禁事实。
            Err(error) => error,
            // 成功会把分数几何错误提升到整数 shape。
            Ok(_) => panic!("fractional additive offset must be rejected"),
        };
        // 拒绝原因必须保持在不能等价 lowering 的类型边界。
        assert_eq!(error.code(), crate::core::Errc::NotImplemented);
        // 失败前只能保留初始 clear，不能偷偷追加 Native 或 CPU segment。
        assert_eq!(
            canvas
                .encoder
                .as_ref()
                .map(|encoder| encoder.commands().len()),
            Some(1)
        );
    }

    // 继续在同一测试模块内加载纯平移 transform 的独立回归测试。
    include!("canvas2d_test_tail.rs");

    // 继续在同一测试模块内加载 Additive opacity 的独立回归测试。
    include!("canvas2d_opacity_tests.rs");
}
