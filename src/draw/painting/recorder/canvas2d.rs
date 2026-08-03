//! [`FrameRecordingCanvas`] 的 `Canvas2D` 实现 — recorder 子模块。
//!
//! 每个绘制原语选择 native / scratch / direct 三条路径之一；仅当语义等价
//! 证明成立才允许 native 或 direct 下放。

use crate::core::Rect;
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::types::{BlendMode, GradientDirection, Radius, Transform};
use crate::draw::painting::{
    FrameEncoder, FrameGlyphBlit, FrameImage, FrameOpacity, FrameRadius, FrameRasterOp, FrameRect,
    FrameSampledRect, FrameStrokeRect, FrameStrokeWidth,
};
use crate::draw::raster::shared_rasterizer::SharedRasterizer;
use crate::draw::{Canvas2D, Color};
use std::sync::Arc;

use super::canvas::FrameRecordingCanvas;
use super::geometry::{frame_encoder_error, pack_visible_scratch_tile, rect_to_frame};

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
        let additive_radius = if self.blend_mode == BlendMode::Additive {
            match radius.map(FrameRadius::new).transpose() {
                Ok(radius) => radius,
                Err(error) => {
                    self.remember_error(frame_encoder_error(error));
                    return;
                }
            }
        } else {
            None
        };
        if self.can_emit_native_additive_fill(rect) {
            if let Err(error) = self.flush_scratch().and_then(|()| {
                let rect =
                    FrameRect::new(rect.x as i32, rect.y as i32, rect.w as i32, rect.h as i32);
                let operation = match additive_radius {
                    Some(radius) => FrameRasterOp::FillRoundedRectAdditive {
                        rect,
                        color,
                        radius,
                    },
                    None => FrameRasterOp::FillRectAdditive { rect, color },
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
        let bounds = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        if r.is_finite() && r > 0.0 && self.native_src_over_rects(bounds).is_some() {
            // A circle is exactly the shared rounded-rect SDF with a square
            // extent and every corner radius equal to half that extent.
            self.fill_rect(bounds, color, Some(Radius::uniform(r)));
            return;
        }
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
        if r.is_finite() && r > 0.0 && self.native_src_over_rects(bounds).is_some() {
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
