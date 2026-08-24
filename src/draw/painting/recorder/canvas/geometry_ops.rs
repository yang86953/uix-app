// 拆分自 canvas.rs：native 保真下放准入与 picture 几何换算。
// 引入矩形、混合模式、圆角与帧编码载荷类型。
use crate::core::Rect;
use crate::draw::geometry::types::{BlendMode, Radius};
use crate::draw::painting::{FrameGlyphBlit, FrameRect, FrameSampledRect};
use crate::draw::{Canvas2D, Color};
use std::sync::Arc;
// 引入被扩展的录制画布与共享帧几何换算。
use super::super::canvas::FrameRecordingCanvas;
use super::super::geometry::rect_to_frame;

// 为 FrameRecordingCanvas 提供保真下放准入与几何换算方法。
impl FrameRecordingCanvas {
    pub(crate) fn native_src_over_rects(&self, rect: Rect) -> Option<(FrameRect, FrameRect)> {
        if !self.scratch.current_transform().is_identity() {
            return None;
        }

        // 与 SoftwareRasterizer 的 identity-transform 填充路径完全一致：
        // offset 只改变 x/y，width/height 保留原始 f32 值。
        let (offset_x, offset_y) = self.scratch.offset();
        let mapped = Rect::new(rect.x + offset_x, rect.y + offset_y, rect.w, rect.h);
        let right = f64::from(mapped.x) + f64::from(mapped.w);
        let bottom = f64::from(mapped.y) + f64::from(mapped.h);
        if !mapped.x.is_finite()
            || !mapped.y.is_finite()
            || !mapped.w.is_finite()
            || !mapped.h.is_finite()
            || mapped.x.fract() != 0.0
            || mapped.y.fract() != 0.0
            || mapped.w.fract() != 0.0
            || mapped.h.fract() != 0.0
            || mapped.x < 0.0
            || mapped.y < 0.0
            || mapped.w <= 0.0
            || mapped.h <= 0.0
            || right > f64::from(self.width)
            || bottom > f64::from(self.height)
        {
            return None;
        }
        Some((
            rect_to_frame(mapped).ok()?,
            self.native_src_over_fill_clip()?,
        ))
    }

    /// identity transform 下返回可直达的字形整数位置与 clip。
    pub(crate) fn native_src_over_glyph(&self, x: i32, y: i32) -> Option<(FrameRect, i32, i32)> {
        if !self.scratch.current_transform().is_identity() {
            return None;
        }
        let (offset_x, offset_y) = self.scratch.offset();
        if !offset_x.is_finite()
            || !offset_y.is_finite()
            || offset_x.fract() != 0.0
            || offset_y.fract() != 0.0
        {
            return None;
        }
        let x = (x as f32 + offset_x) as i32;
        let y = (y as f32 + offset_y) as i32;
        Some((self.native_src_over_fill_clip()?, x, y))
    }

    /// 返回当前可直达的 SrcOver 矩形 clip（存在路径 mask 或非 SrcOver blend 时 None）。
    pub(crate) fn native_src_over_fill_clip(&self) -> Option<FrameRect> {
        if self.scratch.has_clip_mask()
            || !matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver)
        {
            return None;
        }
        let clip = rect_to_frame(self.scratch.current_clip()).ok()?;
        Some(
            clip.intersection(FrameRect::new(0, 0, self.width, self.height))
                .unwrap_or(FrameRect::new(0, 0, 0, 0)),
        )
    }

    /// 录制字形：可直达时走 Native BlitGlyphs，否则进入 sampled scratch。
    pub(crate) fn record_glyph_shared(
        &mut self,
        x: i32,
        y: i32,
        coverage: Arc<[u8]>,
        width: usize,
        height: usize,
        color: Color,
    ) {
        if let Some((clip, native_x, native_y)) = self.native_src_over_glyph(x, y) {
            let native_color = crate::draw::raster::rasterizer::color_with_glyph_opacity(
                color,
                self.scratch.opacity(),
            );
            if native_color.a == 0 {
                return;
            }
            let Ok(glyph) = FrameGlyphBlit::new(
                native_x,
                native_y,
                Arc::clone(&coverage),
                width,
                height,
                native_color,
            ) else {
                // 历史 void Canvas API 把空或畸形 glyph coverage 视为 no-op。
                return;
            };
            if clip.width <= 0 || clip.height <= 0 {
                return;
            }
            if let Err(error) = self.flush_scratch().and_then(|()| {
                self.encoder_mut()?.native_glyph(glyph, clip);
                Ok(())
            }) {
                self.remember_error(error);
            }
            return;
        }
        let Ok(glyph) = FrameGlyphBlit::new(x, y, coverage, width, height, color) else {
            return;
        };
        let bounds = Rect::new(x as f32, y as f32, width as f32, height as f32);
        // 字形只产生源贡献，因此 Additive 可进入可结合的 sampled scratch。
        self.draw_cpu_source(bounds, 1.0, move |scratch| {
            // 共享软件字形负责 offset 后仿射、coverage、clip、opacity 与 blend。
            scratch.blit_glyph(x, y, glyph.coverage().as_ref(), width, height, color)
        });
    }

    /// Additive 轴对齐矩形只进入 Native 命令，并返回已验证的 surface-space 几何与整数 clip。
    pub(crate) fn native_additive_shape(
        &self,
        rect: Rect,
        color: Color,
        radius: Option<Radius>,
    ) -> Option<(FrameRect, FrameRect, Color, Option<Radius>)> {
        // 读取当前 transform，并拆出单位正交准入需要的六个分量。
        let transform = self.scratch.current_transform();
        // 分离线性部分与 surface 平移量，避免把缩放或剪切误判为等距变换。
        let [a, b, transform_x, c, d, transform_y] = transform.m;
        // 保留坐标轴方向或镜像时，两个主对角分量必须分别为正负一。
        let preserves_axes = b == 0.0 && c == 0.0 && a.abs() == 1.0 && d.abs() == 1.0;
        // 交换坐标轴时，两个副对角分量必须分别为正负一。
        let swaps_axes = a == 0.0 && d == 0.0 && b.abs() == 1.0 && c.abs() == 1.0;
        // 两类有符号轴置换共同组成不会改变长度和抗锯齿尺度的单位正交集合。
        let is_unit_orthogonal = preserves_axes || swaps_axes;
        // 读取软件路径用于缩放 premultiplied 颜色的同一全局 opacity。
        let opacity = self.scratch.opacity();
        // 只接受能够由固定 Additive shape pipeline 精确表达的画布状态。
        if self.blend_mode != BlendMode::Additive
            || self.scratch.has_clip_mask()
            || !is_unit_orthogonal
            || !opacity.is_finite()
            || !(0.0..=1.0).contains(&opacity)
        {
            // 其余状态继续沿既有 deferred typed failure 边界处理。
            return None;
        }
        // 取得 SoftwareRasterizer identity 路径同源的像素平移量。
        let (offset_x, offset_y) = self.scratch.offset();
        // 只有有限整数 offset 才能无损进入 FrameEncoder 整数几何。
        if !offset_x.is_finite()
            || !offset_y.is_finite()
            || offset_x.fract() != 0.0
            || offset_y.fract() != 0.0
            || !transform_x.is_finite()
            || !transform_y.is_finite()
            || transform_x.fract() != 0.0
            || transform_y.fract() != 0.0
        {
            // 分数或非有限 offset/transform 平移继续沿 typed failure 边界处理。
            return None;
        }
        // 负尺寸不能借由 transform_rect 的 AABB 归一化伪装成合法图元。
        if !rect.w.is_finite() || !rect.h.is_finite() || rect.w <= 0.0 || rect.h <= 0.0 {
            // 保持既有非法几何拒绝边界。
            return None;
        }
        // 与软件 map_rect 顺序一致：先加 offset，再执行完整单位正交 transform。
        let mapped = transform.transform_rect(Rect::new(
            // 像素 offset 在本地 x 坐标上先行生效。
            rect.x + offset_x,
            // 像素 offset 在本地 y 坐标上先行生效。
            rect.y + offset_y,
            // 单位正交变换不会缩放本地宽度。
            rect.w,
            // 单位正交变换不会缩放本地高度。
            rect.h,
        ));
        // 映射后几何必须是完整位于 surface 内的有限正整数矩形。
        let rect = rect_to_frame(mapped).ok()?;
        // 当前纵切不放宽越界或负尺寸几何。
        if !rect.is_within(self.width, self.height) {
            // 无法证明等价时拒绝提升。
            return None;
        }
        // 矩形 clip 必须同样能够无损转换为 FrameEncoder 整数坐标。
        let clip = rect_to_frame(self.scratch.current_clip()).ok()?;
        // 把调用方裁剪限制到真实 surface；完全不可见时保留显式空 clip。
        let clip = clip
            .intersection(FrameRect::new(0, 0, self.width, self.height))
            .unwrap_or(FrameRect::new(0, 0, 0, 0));
        // 按 CPU 路径顺序把 opacity 折进 premultiplied 颜色并编码回 Color。
        let color =
            crate::draw::raster::rasterizer::color_with_premultiplied_opacity(color, opacity);
        // 按线性变换把本地四角半径重排到映射后的 surface 矩形四角。
        let radius = radius.map(|radius| Self::unit_orthogonal_radius(radius, a, b, c, d));
        // 返回同一录制时刻的几何、裁剪、位精确颜色与角位事实。
        Some((rect, clip, color, radius))
    }

    /// 把本地 `tl/tr/br/bl` 半径重排到单位正交变换后的 surface 矩形四角。
    pub(crate) fn unit_orthogonal_radius(radius: Radius, a: f32, b: f32, c: f32, d: f32) -> Radius {
        // 依照 Radius 的固定字段顺序保存四个本地角载荷。
        let source = [radius.tl, radius.tr, radius.br, radius.bl];
        // 使用单位正方形四角表达与具体矩形尺寸和平移无关的角位映射。
        let local_corners = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        // 只应用已通过准入检查的线性部分，得到四个映射角。
        let mapped_corners = local_corners.map(|(x, y)| (a * x + b * y, c * x + d * y));
        // 找出映射矩形的左边界，用于区分左、右角。
        let min_x = mapped_corners
            .iter()
            .map(|(x, _)| *x)
            .fold(f32::INFINITY, f32::min);
        // 找出映射矩形的上边界，用于区分上、下角。
        let min_y = mapped_corners
            .iter()
            .map(|(_, y)| *y)
            .fold(f32::INFINITY, f32::min);
        // 按 surface 的 tl/tr/br/bl 顺序准备重排结果。
        let mut mapped = [0.0; 4];
        // 每个本地角恰好映射到一个 surface 角。
        for ((x, y), value) in mapped_corners.into_iter().zip(source) {
            // 用相对最小边界的位置选择目标角索引。
            let index = match (x == min_x, y == min_y) {
                // 左上角保持 Radius 的第一个槽位。
                (true, true) => 0,
                // 右上角写入第二个槽位。
                (false, true) => 1,
                // 右下角写入第三个槽位。
                (false, false) => 2,
                // 左下角写入第四个槽位。
                (true, false) => 3,
            };
            // 保存原始半径数值，仅改变其所属角位。
            mapped[index] = value;
        }
        // 以公开语义顺序重建供 FrameRadius 验证的半径。
        Radius {
            // 写回映射后的左上半径。
            tl: mapped[0],
            // 写回映射后的右上半径。
            tr: mapped[1],
            // 写回映射后的右下半径。
            br: mapped[2],
            // 写回映射后的左下半径。
            bl: mapped[3],
        }
    }

    pub(crate) fn direct_picture_rects(
        &self,
        src: Rect,
        dst: Rect,
    ) -> Option<(FrameRect, FrameRect)> {
        if self.scratch.opacity() != 1.0 {
            return None;
        }
        self.direct_picture_geometry(src, dst)
    }

    /// 整数 1:1 splice 几何（仍要求整像素 src/dst）。
    ///
    /// Additive 不参与 splice 收窄；采样路径见 [`Self::sampled_picture_geometry`]。
    pub(crate) fn direct_picture_geometry(
        &self,
        src: Rect,
        dst: Rect,
    ) -> Option<(FrameRect, FrameRect)> {
        let (offset_x, offset_y) = self.scratch.offset();
        if self.blend_mode == BlendMode::Additive
            || self.scratch.has_clip_mask()
            || !offset_x.is_finite()
            || !offset_y.is_finite()
            || offset_x.fract() != 0.0
            || offset_y.fract() != 0.0
            || !self.scratch.current_transform().is_identity()
        {
            return None;
        }
        let src = rect_to_frame(src).ok()?;
        let dst =
            rect_to_frame(Rect::new(dst.x + offset_x, dst.y + offset_y, dst.w, dst.h)).ok()?;
        let clip = self.scratch.current_clip();
        if clip == self.full_rect() && dst.is_within(self.width, self.height) {
            return Some((src, dst));
        }
        if src.width != dst.width || src.height != dst.height {
            return None;
        }
        let clip = rect_to_frame(clip).ok()?;
        let left = dst.x.max(clip.x);
        let top = dst.y.max(clip.y);
        let right = dst
            .x
            .saturating_add(dst.width)
            .min(clip.x.saturating_add(clip.width));
        let bottom = dst
            .y
            .saturating_add(dst.height)
            .min(clip.y.saturating_add(clip.height));
        if left >= right || top >= bottom {
            return None;
        }
        let clipped_dst = FrameRect::new(left, top, right - left, bottom - top);
        let clipped_src = FrameRect::new(
            src.x.saturating_add(left - dst.x),
            src.y.saturating_add(top - dst.y),
            clipped_dst.width,
            clipped_dst.height,
        );
        Some((clipped_src, clipped_dst))
    }

    /// GPU 纹理采样路径：整数源 crop + 浮点目标（允许亚像素 / 缩放）。
    ///
    /// Additive 父 blend 亦允许：命令携带 `additive`，严格 GPU 用 One+One
    /// 纹理 pipeline；hybrid soft 仍会在执行端走参考 tile（需可读目标）。
    pub(crate) fn sampled_picture_geometry(
        &self,
        src: Rect,
        dst: Rect,
    ) -> Option<(FrameRect, FrameSampledRect)> {
        let (offset_x, offset_y) = self.scratch.offset();
        if self.scratch.has_clip_mask()
            || !offset_x.is_finite()
            || !offset_y.is_finite()
            || !self.scratch.current_transform().is_identity()
        {
            return None;
        }
        // Additive 下跳过依赖 SrcOver splice 的整数裁剪分支，整块源 crop
        // 交给执行端 Additive 纹理四边形 / 参考采样。
        let allow_integer_clip = self.blend_mode != BlendMode::Additive;
        let src = rect_to_frame(src).ok()?;
        if src.width <= 0 || src.height <= 0 {
            return None;
        }
        let dest_x = dst.x + offset_x;
        let dest_y = dst.y + offset_y;
        if !dest_x.is_finite()
            || !dest_y.is_finite()
            || !dst.w.is_finite()
            || !dst.h.is_finite()
            || dst.w <= 0.0
            || dst.h <= 0.0
        {
            return None;
        }
        // 整数 1:1 且 clip 可精确裁剪时优先收窄，便于 soft tile / splice 复用。
        let one_to_one = dst.w == src.width as f32 && dst.h == src.height as f32;
        let integer_placement = offset_x.fract() == 0.0
            && offset_y.fract() == 0.0
            && dest_x.fract() == 0.0
            && dest_y.fract() == 0.0
            && dst.w.fract() == 0.0
            && dst.h.fract() == 0.0;
        if allow_integer_clip && one_to_one && integer_placement {
            if let Some((clipped_src, clipped_dst)) = self.direct_picture_geometry(
                Rect::new(
                    src.x as f32,
                    src.y as f32,
                    src.width as f32,
                    src.height as f32,
                ),
                Rect::new(dst.x, dst.y, dst.w, dst.h),
            ) {
                return Some((clipped_src, FrameSampledRect::from_integer(clipped_dst)));
            }
        }
        // 亚像素落点或缩放：整块源 crop，裁剪交给执行端 scissor / 采样。
        if dest_x + dst.w <= 0.0
            || dest_y + dst.h <= 0.0
            || dest_x >= self.width as f32
            || dest_y >= self.height as f32
        {
            return None;
        }
        let sampled = FrameSampledRect::from_parts(dest_x, dest_y, dst.w, dst.h).ok()?;
        Some((src, sampled))
    }

    /// 返回完整 surface 矩形（f32）。
    pub(crate) fn full_rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width as f32, self.height as f32)
    }
}
