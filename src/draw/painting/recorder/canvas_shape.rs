//! `FrameRecordingCanvas` 的 SrcOver shape 原生准入契约。

// 引入真实浮点矩形。
use crate::core::Rect;
// 引入允许原生 SrcOver 的混合模式。
use crate::draw::geometry::types::Radius;
// 引入整数裁剪和亚像素 shape 矩形。
use crate::draw::painting::{
    FrameGlyphOutline, FrameRadius, FrameRasterOp, FrameRect, FrameSampledRect, FrameStrokeWidth,
};
// 引入公开画布颜色值。
use crate::draw::{Canvas2D, Color};
// 引入共享字体轮廓所有权。
use std::sync::Arc;

// 引入录制画布状态所有者。
use super::canvas::FrameRecordingCanvas;
// 引入严格整数矩形转换。

// 为录制画布实现轴对齐 SrcOver shape 的准入证明。
impl FrameRecordingCanvas {
    // 尝试把普通 SrcOver 亚像素填充直接记录为 GPU 原生命令。
    pub(super) fn record_src_over_subpixel_fill(
        // 可变借用当前录制状态。
        &mut self,
        // 接收本地矩形。
        rect: Rect,
        // 接收直通颜色。
        color: Color,
        // 接收可选圆角。
        radius: Option<Radius>,
    ) -> bool {
        // 验证亚像素几何与整数裁剪。
        let Some((rect, clip)) = self.native_src_over_subpixel_rect(rect) else {
            // 准入失败时允许调用方继续软件路径。
            return false;
        };
        // 验证圆角值，非法半径保持既有软件行为。
        let Ok(radius) = radius.map(FrameRadius::new).transpose() else {
            // 准入失败时允许调用方继续软件路径。
            return false;
        };
        // 计算包含画布 opacity 的预乘语义颜色。
        let color = crate::draw::raster::rasterizer::color_with_premultiplied_opacity(
            // 保留调用方颜色。
            color,
            // 应用当前画布透明度。
            self.scratch.opacity(),
        );
        // 空裁剪或全透明颜色是安全 no-op。
        if clip.is_empty() || color.a == 0 {
            // 报告命令已经完整处理。
            return true;
        }
        // 在 painter-order barrier 后记录亚像素填充。
        if let Err(error) = self.flush_scratch().and_then(|()| {
            // 追加普通 SrcOver 亚像素填充命令。
            self.encoder_mut()?
                .native(FrameRasterOp::FillRoundedRectSubpixel {
                    // 保存真实亚像素矩形。
                    rect,
                    // 保存 opacity 后的颜色。
                    color,
                    // 无圆角时使用规范零半径。
                    radius: radius.unwrap_or_else(FrameRadius::zero),
                    // 保存整数 surface 裁剪。
                    clip,
                });
            // 报告命令记录成功。
            Ok(())
        }) {
            // 延迟报告 flush 或 encoder 状态错误。
            self.remember_error(error);
        }
        // 几何已经由原生命令路径完整处理。
        true
    }

    // 尝试把普通 SrcOver 亚像素描边直接记录为 GPU 原生命令。
    pub(super) fn record_src_over_subpixel_stroke(
        // 可变借用当前录制状态。
        &mut self,
        // 接收本地矩形。
        rect: Rect,
        // 接收直通颜色。
        color: Color,
        // 接收描边宽度。
        width: f32,
        // 接收可选圆角。
        radius: Option<Radius>,
    ) -> bool {
        // 验证亚像素几何与整数裁剪。
        let Some((rect, clip)) = self.native_src_over_subpixel_rect(rect) else {
            // 准入失败时允许调用方继续软件路径。
            return false;
        };
        // 同时验证圆角和正有限描边宽度。
        let (Ok(radius), Ok(line_width)) = (
            // 转换可选圆角。
            radius.map(FrameRadius::new).transpose(),
            // 转换描边宽度。
            FrameStrokeWidth::new(width),
        ) else {
            // 准入失败时允许调用方继续软件路径。
            return false;
        };
        // 计算包含画布 opacity 的预乘语义颜色。
        let color = crate::draw::raster::rasterizer::color_with_premultiplied_opacity(
            // 保留调用方颜色。
            color,
            // 应用当前画布透明度。
            self.scratch.opacity(),
        );
        // 空裁剪或全透明颜色是安全 no-op。
        if clip.is_empty() || color.a == 0 {
            // 报告命令已经完整处理。
            return true;
        }
        // 在 painter-order barrier 后记录亚像素描边。
        if let Err(error) = self.flush_scratch().and_then(|()| {
            // 追加普通 SrcOver 亚像素描边命令。
            self.encoder_mut()?
                .native(FrameRasterOp::StrokeRoundedRectSubpixel {
                    // 保存真实亚像素矩形。
                    rect,
                    // 保存 opacity 后的颜色。
                    color,
                    // 无圆角时使用规范零半径。
                    radius: radius.unwrap_or_else(FrameRadius::zero),
                    // 保存已经验证的描边宽度。
                    line_width,
                    // 保存整数 surface 裁剪。
                    clip,
                });
            // 报告命令记录成功。
            Ok(())
        }) {
            // 延迟报告 flush 或 encoder 状态错误。
            self.remember_error(error);
        }
        // 几何已经由原生命令路径完整处理。
        true
    }

    // 尝试把字体轮廓记录为 GPU MSDF 原生命令。
    pub(super) fn record_glyph_outline_shared(
        // 可变借用当前录制状态。
        &mut self,
        // 接收字形水平位置。
        x: i32,
        // 接收字形垂直位置。
        y: i32,
        // 接收共享轮廓边列表。
        edges: Arc<[f32]>,
        // 接收目标宽度。
        width: usize,
        // 接收目标高度。
        height: usize,
        // 接收直通颜色。
        color: Color,
    ) -> bool {
        // 验证 identity transform、整数 offset 与整数裁剪。
        let Some((clip, x, y)) = self.native_src_over_glyph(x, y) else {
            // 准入失败时允许调用方继续 coverage 路径。
            return false;
        };
        // 计算包含画布 opacity 的字形颜色。
        let color = crate::draw::raster::rasterizer::color_with_glyph_opacity(
            // 保留调用方颜色。
            color,
            // 应用当前画布透明度。
            self.scratch.opacity(),
        );
        // 验证轮廓 ABI 和目标尺寸。
        let Some(glyph) = FrameGlyphOutline::new(x, y, edges, width, height, color) else {
            // 准入失败时允许调用方继续 coverage 路径。
            return false;
        };
        // 空裁剪或全透明颜色是安全 no-op。
        if clip.is_empty() || color.a == 0 {
            // 报告命令已经完整处理。
            return true;
        }
        // 在 painter-order barrier 后记录字形轮廓。
        if let Err(error) = self.flush_scratch().and_then(|()| {
            // 追加单字形轮廓批次，后续可由 encoder 合并相邻 clip。
            self.encoder_mut()?
                .native(FrameRasterOp::BlitGlyphOutlines {
                    // 保存唯一字形轮廓。
                    glyphs: vec![glyph],
                    // 保存整数 surface 裁剪。
                    clip,
                });
            // 报告命令记录成功。
            Ok(())
        }) {
            // 延迟报告 flush 或 encoder 状态错误。
            self.remember_error(error);
        }
        // 字形已经由原生轮廓路径完整处理。
        true
    }

    // 为 identity transform 下的亚像素矩形建立 GPU shape 准入事实。
    pub(super) fn native_src_over_subpixel_rect(
        // 借用当前录制状态。
        &self,
        // 接收调用方本地矩形。
        rect: Rect,
    ) -> Option<(FrameSampledRect, FrameRect)> {
        // 亚像素 shape 仍不接受旋转、缩放或斜切。
        if !self.scratch.current_transform().is_identity() {
            // 拒绝未经证明的变换几何。
            return None;
        }
        // 读取只平移位置的共享 offset。
        let (offset_x, offset_y) = self.scratch.offset();
        // 构造 surface 空间矩形。
        let mapped = Rect::new(rect.x + offset_x, rect.y + offset_y, rect.w, rect.h);
        // 计算右边界并扩大精度防止溢出误判。
        let right = f64::from(mapped.x) + f64::from(mapped.w);
        // 计算下边界并扩大精度防止溢出误判。
        let bottom = f64::from(mapped.y) + f64::from(mapped.h);
        // 亚像素原生命令只接受完整 surface 内的正有限几何。
        if !mapped.x.is_finite()
            || !mapped.y.is_finite()
            || !mapped.w.is_finite()
            || !mapped.h.is_finite()
            || mapped.x < 0.0
            || mapped.y < 0.0
            || mapped.w <= 0.0
            || mapped.h <= 0.0
            || right > f64::from(self.width)
            || bottom > f64::from(self.height)
        {
            // 拒绝越界或无效几何。
            return None;
        }
        // 构造保持 f32 位模式的亚像素值类型。
        let sampled = FrameSampledRect::from_parts(mapped.x, mapped.y, mapped.w, mapped.h).ok()?;
        // 返回亚像素 shape 和统一 surface 裁剪。
        Some((sampled, self.native_src_over_fill_clip()?))
    }
}
