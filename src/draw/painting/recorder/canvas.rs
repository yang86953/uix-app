//! 主画布录制器 — recorder 子模块。
//!
//! [`FrameRecordingCanvas`] 是 `Canvas2D` 的录制实现：绘制操作要么降级为
//! 已证明的原生命令，要么落入共享软件光栅 scratch 并在 flush 时编码为透明
//! SrcOver CPU segment 或 Additive sampled segment。

use crate::core::{Errc, Error, Rect};
// Additive 正交变换准入需要判断圆角是否在旋转或镜像下保持不变。
use crate::draw::geometry::types::{BlendMode, Radius};
use crate::draw::painting::{
    FrameEncoder, FrameGlyphBlit, FrameImage, FrameOpacity, FrameRasterOp, FrameRect,
    FrameSampledRect,
};
use crate::draw::raster::pixel_surface::PixelSurface;
use crate::draw::raster::shared_rasterizer::SharedRasterizer;
use crate::draw::{Canvas2D, Color};
use std::sync::Arc;

use super::geometry::{
    frame_encoder_error, pack_visible_scratch_tile, rect_to_frame, surface_pack_bounds,
    union_frame_rect,
};

/// State-preserving CPU scratch rasterizer. Consecutive CPU draws accumulate
/// in scratch and flush at painter-order barriers (native / Picture / finish),
/// so glyphs and rounded fills share one packed CpuSegment instead of
/// re-scanning the window after every op.
pub(super) struct FrameRecordingCanvas {
    pub(super) scratch: SharedRasterizer,
    pub(super) encoder: Option<FrameEncoder>,
    pub(super) blend_mode: BlendMode,
    pub(super) blend_stack: Vec<BlendMode>,
    pub(super) scratch_dirty: bool,
    /// 当前 scratch 批次是否必须以 Additive sampled texture 合成。
    pub(super) scratch_additive: bool,
    /// Surface-space AABB covering pixels written since the last flush.
    /// Pack scans only this region (plus AA pad) instead of the full window.
    pub(super) scratch_pack_bounds: Option<FrameRect>,
    pub(super) deferred_error: Option<Error>,
    pub(super) width: i32,
    pub(super) height: i32,
}

impl FrameRecordingCanvas {
    pub(super) fn new(width: i32, height: i32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let mut scratch = SharedRasterizer::new(PixelSurface::one_pixel());
        scratch.reset_state_for_extent(width, height);
        Self {
            scratch,
            encoder: None,
            blend_mode: BlendMode::default(),
            blend_stack: Vec::new(),
            scratch_dirty: false,
            scratch_additive: false,
            scratch_pack_bounds: None,
            deferred_error: None,
            width,
            height,
        }
    }

    pub(super) fn prepare_resize(width: i32, height: i32) -> Result<(i32, i32), Error> {
        let width = width.max(1);
        let height = height.max(1);
        PixelSurface::validate_extent(width, height)?;
        Ok((width, height))
    }

    pub(super) fn commit_resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        self.scratch
            .replace_surface_preserving_state(PixelSurface::one_pixel());
        self.scratch.reset_state_for_extent(width, height);
        self.encoder = None;
        self.blend_mode = BlendMode::default();
        self.blend_stack.clear();
        self.scratch_dirty = false;
        self.scratch_additive = false;
        self.scratch_pack_bounds = None;
        self.deferred_error = None;
    }

    pub(super) fn begin_recording(&mut self, clear_target: bool) -> Result<(), Error> {
        // A successful flush clears every touched scratch pixel. Reuse that
        // allocation instead of reallocating and zeroing the full window each
        // frame. An abandoned recording may have unflushed pixels, so only
        // that recovery boundary pays for a conservative full clear.
        if self.encoder.is_some() || self.scratch_dirty || self.deferred_error.is_some() {
            self.scratch.surface_mut().clear_all();
        }
        self.scratch.reset_state_for_extent(self.width, self.height);
        self.blend_mode = BlendMode::default();
        self.blend_stack.clear();
        self.scratch_dirty = false;
        self.scratch_additive = false;
        self.scratch_pack_bounds = None;
        self.deferred_error = None;
        let mut encoder =
            FrameEncoder::new(self.width, self.height).map_err(frame_encoder_error)?;
        if clear_target {
            encoder.clear(Color::transparent());
        }
        self.encoder = Some(encoder);
        Ok(())
    }

    pub(super) fn finish_recording(&mut self) -> Result<FrameEncoder, Error> {
        self.flush_scratch()?;
        if let Some(error) = self.deferred_error.take() {
            return Err(error);
        }
        self.encoder.take().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "FrameEncoder recording was not started before finish",
            )
        })
    }

    pub(super) fn flush_recording(&mut self) -> Result<(), Error> {
        self.flush_scratch()?;
        if let Some(error) = self.deferred_error.take() {
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn abandon_recording(&mut self) {
        if (
            self.scratch.surface().width(),
            self.scratch.surface().height(),
        ) == (1, 1)
            && (self.scratch_dirty || self.encoder.is_some() || self.deferred_error.is_some())
        {
            self.scratch.surface_mut().clear_all();
        }
        self.scratch.reset_state_for_extent(self.width, self.height);
        self.encoder = None;
        self.blend_mode = BlendMode::default();
        self.blend_stack.clear();
        self.scratch_dirty = false;
        self.scratch_additive = false;
        self.scratch_pack_bounds = None;
        self.deferred_error = None;
        self.release_scratch_allocation();
    }

    pub(super) fn release_scratch_allocation(&mut self) {
        if (
            self.scratch.surface().width(),
            self.scratch.surface().height(),
        ) != (1, 1)
        {
            self.scratch
                .replace_surface_preserving_state(PixelSurface::one_pixel());
        }
        self.scratch.reset_state_for_extent(self.width, self.height);
    }

    pub(super) fn retained_memory_usage(&self) -> usize {
        self.scratch.memory_usage().saturating_add(
            self.encoder
                .as_ref()
                .map(FrameEncoder::retained_memory_usage)
                .unwrap_or(0),
        )
    }

    pub(super) fn record_validated_commands(
        &mut self,
        commands: Vec<crate::draw::painting::FrameCommand>,
    ) -> Result<(), Error> {
        self.flush_scratch()?;
        self.encoder_mut()?
            .append_validated_commands(commands)
            .map_err(frame_encoder_error)?;
        Ok(())
    }

    pub(super) fn record_picture_blit(
        &mut self,
        image: FrameImage,
        src: Rect,
        dst: Rect,
    ) -> Result<(), Error> {
        self.flush_scratch()?;
        if let Some((src, dst)) = self.sampled_picture_geometry(src, dst) {
            let opacity = FrameOpacity::from_canvas(self.scratch.opacity());
            let additive = self.blend_mode == BlendMode::Additive;
            self.encoder_mut()?
                .blit_picture_with_opacity_blend(image, src, dst, opacity, additive);
            return Ok(());
        }
        // Additive 下不得走透明 scratch 再 SrcOver 上传。
        if self.blend_mode == BlendMode::Additive {
            let error = Error::new(
                Errc::NotImplemented,
                "FrameEncoder recording cannot faithfully lower destination-dependent Additive Picture blit geometry",
            );
            self.remember_error(error.clone());
            return Err(error);
        }
        self.ensure_scratch()?;
        self.scratch
            .blit_image(image.pixels(), image.width(), src, dst);
        self.note_scratch_bounds(dst, 1.0);
        self.scratch_dirty = true;
        self.flush_scratch()
    }

    pub(super) fn record_direct_image_blit(
        &mut self,
        pixels: &[u32],
        source_width: i32,
        source_rect: Rect,
        destination_rect: Rect,
    ) -> Result<bool, Error> {
        let canvas_opacity = self.scratch.opacity();
        if !canvas_opacity.is_finite() {
            return Ok(false);
        }
        let opacity = FrameOpacity::from_canvas(canvas_opacity);
        if opacity.is_transparent() {
            return Ok(true);
        }
        // Additive raw image 优先尝试复用现有 sampled textured pipeline。
        if self.blend_mode == BlendMode::Additive {
            // 只有完整 clip 与 identity transform 会在辅助模块中安全直达。
            return self.record_additive_image_blit(
                pixels,
                source_width,
                source_rect,
                destination_rect,
                opacity,
            );
        }
        let Ok(source_stride) = usize::try_from(source_width) else {
            return Ok(false);
        };
        if source_stride == 0 {
            return Ok(false);
        }
        let Ok(source_height) = i32::try_from(pixels.len() / source_stride) else {
            return Ok(false);
        };
        let Some((source, destination)) =
            self.direct_picture_geometry(source_rect, destination_rect)
        else {
            return Ok(false);
        };
        if source.width != destination.width
            || source.height != destination.height
            || !source.is_within(source_width, source_height)
        {
            return Ok(false);
        }

        let pixel_count = usize::try_from(i64::from(source.width) * i64::from(source.height))
            .map_err(|_| {
                Error::new(
                    Errc::GraphicsOutOfMemory,
                    "direct image blit crop exceeds addressable memory",
                )
            })?;
        let mut retained = Vec::new();
        retained.try_reserve_exact(pixel_count).map_err(|error| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!(
                    "direct image blit crop {}x{} allocation failed: {error}",
                    source.width, source.height
                ),
            )
        })?;
        let copy_width = source.width as usize;
        for y in source.y..source.y + source.height {
            let row = y as usize * source_stride + source.x as usize;
            retained.extend_from_slice(&pixels[row..row + copy_width]);
        }
        let image =
            FrameImage::new(source.width, source.height, retained).map_err(frame_encoder_error)?;
        let retained_source = FrameRect::new(0, 0, source.width, source.height);
        self.flush_scratch()?;
        self.encoder_mut()?.blit_picture_integer_with_opacity(
            image,
            retained_source,
            destination,
            opacity,
        );
        Ok(true)
    }

    pub(super) fn note_scratch_bounds(&mut self, local: Rect, pad: f32) {
        // 先在本地空间扩展线宽、模糊或抗锯齿边界，使缩放和剪切不会截断像素。
        let local_pad = pad.max(0.0);
        // 构造完整覆盖本地写区的保守矩形。
        let expanded = Rect::new(
            // 扩展本地左边界。
            local.x - local_pad,
            // 扩展本地上边界。
            local.y - local_pad,
            // 同时扩展左右两侧。
            local.w + local_pad * 2.0,
            // 同时扩展上下两侧。
            local.h + local_pad * 2.0,
        );
        // 完整映射扩展后的本地 AABB，保守覆盖任意仿射写区。
        let mapped = self.scratch.map_rect(expanded);
        let Some(bounds) = surface_pack_bounds(
            mapped,
            // 映射后只保留一个设备像素的抗锯齿余量。
            1.0,
            (0.0, 0.0),
            self.scratch.current_clip(),
            self.width,
            self.height,
        ) else {
            return;
        };
        self.scratch_pack_bounds = Some(match self.scratch_pack_bounds {
            Some(prev) => union_frame_rect(prev, bounds),
            None => bounds,
        });
    }

    pub(super) fn draw_cpu(
        &mut self,
        local_bounds: Rect,
        pad: f32,
        draw: impl FnOnce(&mut SharedRasterizer),
    ) {
        if self.deferred_error.is_some() {
            return;
        }
        // Transparent scratch + source-over upload cannot preserve Additive
        // against prior commands; only explicitly promoted destination-dependent Native ops are equivalent.
        if self.blend_mode == BlendMode::Additive {
            self.unsupported_state("destination-dependent Additive blend via CPU segment");
            return;
        }
        // 普通软件操作继续写入 SrcOver CPU segment 批次。
        self.draw_scratch(local_bounds, pad, false, draw);
    }

    /// 把已经证明可结合的 Additive 源贡献累积到透明 scratch，稍后以 Additive
    /// sampled texture 对累计目标合成。
    pub(super) fn draw_additive_cpu(
        &mut self,
        local_bounds: Rect,
        pad: f32,
        draw: impl FnOnce(&mut SharedRasterizer),
    ) {
        // 只允许显式 Additive 调用进入目标相关 sampled 批次。
        if self.blend_mode != BlendMode::Additive {
            // 错误调用保持稳定的 typed failure，而不是改变普通 blend 语义。
            self.unsupported_state("Additive sampled scratch without Additive blend");
            // 禁止继续写入错误批次。
            return;
        }
        // 非有限 opacity 无法稳定烘焙为 premultiplied sampled tile。
        if !self.scratch.opacity().is_finite() {
            // 保留既有 deferred typed failure 契约。
            self.unsupported_state("Additive sampled scratch with non-finite opacity");
            // 禁止把 NaN 转换成静默透明的源贡献。
            return;
        }
        // Additive 填充与描边使用同一目标相关 scratch 批次。
        self.draw_scratch(local_bounds, pad, true, draw);
    }

    /// 根据当前 blend 为只产生源贡献的软件操作选择普通 CPU segment 或
    /// Additive sampled segment。
    pub(super) fn draw_cpu_source(
        &mut self,
        local_bounds: Rect,
        pad: f32,
        draw: impl FnOnce(&mut SharedRasterizer),
    ) {
        // Additive 源贡献必须在最终目标上执行饱和加法。
        if self.blend_mode == BlendMode::Additive {
            // 透明 scratch 只保存可结合的源贡献。
            self.draw_additive_cpu(local_bounds, pad, draw);
        } else {
            // Alpha 与 SrcOver 保持既有 CPU segment 行为。
            self.draw_cpu(local_bounds, pad, draw);
        }
    }

    /// 向同一 blend 的透明 scratch 批次追加一个软件光栅操作。
    fn draw_scratch(
        &mut self,
        local_bounds: Rect,
        pad: f32,
        additive: bool,
        draw: impl FnOnce(&mut SharedRasterizer),
    ) {
        // 已有不同 blend 的像素必须先形成 painter-order barrier。
        if self.scratch_dirty && self.scratch_additive != additive {
            // flush 失败时保留原批次并延迟报告，不能混写后续像素。
            if let Err(error) = self.flush_scratch() {
                // 保存首个稳定错误。
                self.remember_error(error);
                // 停止当前操作。
                return;
            }
        }
        // 录制器已经失败时不再修改 scratch。
        if self.deferred_error.is_some() {
            // 保持首个错误及当前命令流。
            return;
        }
        if let Err(error) = self.ensure_scratch() {
            self.remember_error(error);
            return;
        }
        draw(&mut self.scratch);
        self.note_scratch_bounds(local_bounds, pad);
        self.scratch_dirty = true;
        // 保存本批最终合成需要使用的 blend 事实。
        self.scratch_additive = additive;
        // Defer flush until a painter-order barrier (native op / Picture blit /
        // finish). Per-op flush re-scanned and re-uploaded after every glyph
        // and rounded fill, dominating record time on dense pages.
    }

    pub(super) fn flush_scratch(&mut self) -> Result<(), Error> {
        if !self.scratch_dirty {
            return Ok(());
        }
        let pack_bounds = self.scratch_pack_bounds.take();
        let packed = pack_visible_scratch_tile(
            self.scratch.surface().pixels(),
            self.width,
            self.height,
            pack_bounds,
        );
        if let Some((pixels, dst)) = packed {
            let image =
                FrameImage::new(dst.width, dst.height, pixels).map_err(frame_encoder_error)?;
            let src = FrameRect::new(0, 0, dst.width, dst.height);
            // Additive scratch 已经把每笔 opacity、clip 与 transform 烘焙进源像素。
            if self.scratch_additive {
                // 以 opaque opacity 只执行一次最终目标相关饱和加法。
                self.encoder_mut()?.blit_picture_with_opacity_blend(
                    image,
                    src,
                    FrameSampledRect::from_integer(dst),
                    FrameOpacity::opaque(),
                    true,
                );
            } else {
                // 普通透明 scratch 继续使用既有 SrcOver CPU segment。
                self.encoder_mut()?.cpu_image_segment(image, src, dst);
            }
        }
        // pack_bounds 是本批所有 draw bounds 的并集，不是最后一笔；清理该并集即可
        // 隔离下一批，同时避免每个 painter barrier 都扫完整窗口。
        if let Some(bounds) = pack_bounds.filter(|bounds| {
            let bounded_area = i64::from(bounds.width).saturating_mul(i64::from(bounds.height));
            let surface_area = i64::from(self.width).saturating_mul(i64::from(self.height));
            bounded_area.saturating_mul(2) < surface_area
        }) {
            self.scratch.surface_mut().clear_rect_raw(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
            );
        } else {
            // 大批次连续 clear_all 更快；pixels_mut 等无边界写入也必须保守全清。
            self.scratch.surface_mut().clear_all();
        }
        self.scratch_dirty = false;
        // 空 scratch 不再携带上一批 Additive 事实。
        self.scratch_additive = false;
        Ok(())
    }

    pub(super) fn ensure_scratch(&mut self) -> Result<(), Error> {
        if (
            self.scratch.surface().width(),
            self.scratch.surface().height(),
        ) == (self.width, self.height)
        {
            return Ok(());
        }
        let surface = PixelSurface::try_new(self.width, self.height)?;
        self.scratch.replace_surface_preserving_state(surface);
        Ok(())
    }

    pub(super) fn encoder_mut(&mut self) -> Result<&mut FrameEncoder, Error> {
        self.encoder.as_mut().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "FrameEncoder command was recorded outside its frame lifetime",
            )
        })
    }

    pub(super) fn remember_error(&mut self, error: Error) {
        if self.deferred_error.is_none() {
            self.deferred_error = Some(error);
        }
    }

    pub(super) fn native_src_over_rects(&self, rect: Rect) -> Option<(FrameRect, FrameRect)> {
        if !self.scratch.current_transform().is_identity() {
            return None;
        }

        // Match SoftwareRasterizer's identity-transform fill path exactly: offset
        // changes x/y only, while width/height retain their original f32 values.
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

    pub(super) fn native_src_over_glyph(&self, x: i32, y: i32) -> Option<(FrameRect, i32, i32)> {
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

    pub(super) fn native_src_over_fill_clip(&self) -> Option<FrameRect> {
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

    pub(super) fn record_glyph_shared(
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
                self.encoder_mut()?.native(FrameRasterOp::BlitGlyphs {
                    glyphs: vec![glyph],
                    clip,
                });
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
    pub(super) fn native_additive_shape(
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
    pub(super) fn unit_orthogonal_radius(radius: Radius, a: f32, b: f32, c: f32, d: f32) -> Radius {
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

    pub(super) fn direct_picture_rects(
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
    pub(super) fn direct_picture_geometry(
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
    pub(super) fn sampled_picture_geometry(
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

    pub(super) fn full_rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width as f32, self.height as f32)
    }

    pub(super) fn unsupported_state(&mut self, detail: &'static str) {
        self.remember_error(Error::new(
            Errc::NotImplemented,
            format!("FrameEncoder recording cannot faithfully lower {detail}"),
        ));
    }
}
