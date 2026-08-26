//! 主画布录制器 — recorder 子模块。
//!
//! [`FrameRecordingCanvas`] 是 `Canvas2D` 的录制实现：绘制操作要么降级为
//! 已证明的原生命令，要么落入共享软件光栅 scratch 并在 flush 时编码为透明
//! SrcOver CPU segment 或 Additive sampled segment。

use crate::core::{Errc, Error, Rect};
// Additive 正交变换准入需要判断圆角是否在旋转或镜像下保持不变。
use crate::draw::geometry::types::BlendMode;
use crate::draw::painting::{FrameEncoder, FrameImage, FrameOpacity, FrameRect, FrameSampledRect};
use crate::draw::raster::pixel_surface::PixelSurface;
use crate::draw::raster::shared_rasterizer::SharedRasterizer;
use crate::draw::{Canvas2D, Color};

use super::geometry::{
    frame_encoder_error, pack_visible_scratch_tile, surface_pack_bounds, union_frame_rect,
};

// 保真下放准入与 picture 几何换算拆分到独立模块。
mod geometry_ops;

// 小命令流至少允许保留一轮常见 Vec 增长余量。
const MIN_RETAINED_COMMAND_CAPACITY: usize = 16;
// 场景骤减后容量超过当前命令数四倍时释放，避免峰值命令流长期驻留。
const MAX_RETAINED_COMMAND_CAPACITY_RATIO: usize = 4;
// 常见短文本至少保留一轮字形 Vec 增长余量。
const MIN_RETAINED_GLYPH_CAPACITY: usize = 16;
// 字形批次骤减后同样只保留当前规模四倍以内的槽位。
const MAX_RETAINED_GLYPH_CAPACITY_RATIO: usize = 4;
// 常见边框至少保留一个小型描边批次。
const MIN_RETAINED_STROKE_CAPACITY: usize = 4;
// 描边批次骤减后只保留当前规模四倍以内的槽位。
const MAX_RETAINED_STROKE_CAPACITY_RATIO: usize = 4;

/// 状态保持的 CPU scratch 光栅化器。连续的 CPU 绘制累积在 scratch 中，
/// 在 painter-order 屏障（native / Picture / finish）处 flush，使字形与
/// 圆角填充共享一个打包的 CpuSegment，而不是每笔操作后重新扫描窗口。
pub(super) struct FrameRecordingCanvas {
    pub(super) scratch: SharedRasterizer,
    pub(super) encoder: Option<FrameEncoder>,
    /// 同尺寸上一已执行帧归还的空命令缓冲；只保留 Vec 容量，不保留命令载荷。
    pub(super) spare_encoder: Option<FrameEncoder>,
    /// 上一成功帧的命令数，用于减少稳定场景每帧 Vec 扩容。
    pub(super) command_capacity_hint: usize,
    pub(super) blend_mode: BlendMode,
    pub(super) blend_stack: Vec<BlendMode>,
    pub(super) scratch_dirty: bool,
    /// 当前 scratch 批次是否必须以 Additive sampled texture 合成。
    pub(super) scratch_additive: bool,
    /// 上次 flush 以来被写入像素的 surface 空间 AABB。
    /// pack 只扫描该区域（外加抗锯齿余量）而不是整个窗口。
    pub(super) scratch_pack_bounds: Option<FrameRect>,
    pub(super) deferred_error: Option<Error>,
    pub(super) width: i32,
    pub(super) height: i32,
}

impl FrameRecordingCanvas {
    /// 创建最小（1×1）录制画布；目标尺寸在 resize / 录制时按需分配。
    pub(super) fn new(width: i32, height: i32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let mut scratch = SharedRasterizer::new(PixelSurface::one_pixel());
        scratch.reset_state_for_extent(width, height);
        Self {
            scratch,
            encoder: None,
            spare_encoder: None,
            command_capacity_hint: 0,
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

    /// 校验并规范化目标尺寸（至少 1×1）。
    pub(super) fn prepare_resize(width: i32, height: i32) -> Result<(i32, i32), Error> {
        let width = width.max(1);
        let height = height.max(1);
        PixelSurface::validate_extent(width, height)?;
        Ok((width, height))
    }

    /// 提交新尺寸并重置全部录制状态。
    pub(super) fn commit_resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        self.scratch
            .replace_surface_preserving_state(PixelSurface::one_pixel());
        self.scratch.reset_state_for_extent(width, height);
        self.encoder = None;
        self.spare_encoder = None;
        self.blend_mode = BlendMode::default();
        self.blend_stack.clear();
        self.scratch_dirty = false;
        self.scratch_additive = false;
        self.scratch_pack_bounds = None;
        self.deferred_error = None;
    }

    /// 开始一帧录制：重置 scratch 状态并创建新的 `FrameEncoder`。
    pub(super) fn begin_recording(&mut self, clear_target: bool) -> Result<(), Error> {
        // 成功 flush 会清空所有被触及的 scratch 像素。复用该分配，而不是
        // 每帧重新分配并清零整个窗口。被放弃的录制可能残留未 flush 像素，
        // 因此只有该恢复边界需要保守地全量清除。
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
        let mut encoder = match self.spare_encoder.take() {
            Some(encoder) => encoder,
            None => FrameEncoder::with_command_capacity(
                self.width,
                self.height,
                self.command_capacity_hint,
            )
            .map_err(frame_encoder_error)?,
        };
        if clear_target {
            encoder.clear(Color::transparent());
        }
        self.encoder = Some(encoder);
        Ok(())
    }

    /// 结束录制：flush 剩余 scratch 并交出 `FrameEncoder`。
    pub(super) fn finish_recording(&mut self) -> Result<FrameEncoder, Error> {
        self.flush_scratch()?;
        if let Some(error) = self.deferred_error.take() {
            return Err(error);
        }
        let encoder = self.encoder.take().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "FrameEncoder recording was not started before finish",
            )
        })?;
        self.command_capacity_hint = encoder.commands().len();
        Ok(encoder)
    }

    /// 收回同步执行完成的同尺寸编码器；释放命令载荷并有界保留 Vec 容量。
    pub(super) fn recycle_encoder(&mut self, mut encoder: FrameEncoder) {
        let command_count = encoder.commands().len();
        let glyph_batch_len = encoder.max_glyph_batch_len();
        let stroke_batch_len = encoder.max_stroke_batch_len();
        self.command_capacity_hint = command_count;
        let retain_limit = command_count
            .max(MIN_RETAINED_COMMAND_CAPACITY)
            .saturating_mul(MAX_RETAINED_COMMAND_CAPACITY_RATIO);
        if self.encoder.is_some()
            || encoder.width() != self.width
            || encoder.height() != self.height
            || encoder.command_capacity() > retain_limit
        {
            return;
        }
        let glyph_capacity_limit = glyph_batch_len
            .max(MIN_RETAINED_GLYPH_CAPACITY)
            .saturating_mul(MAX_RETAINED_GLYPH_CAPACITY_RATIO);
        let stroke_capacity_limit = stroke_batch_len
            .max(MIN_RETAINED_STROKE_CAPACITY)
            .saturating_mul(MAX_RETAINED_STROKE_CAPACITY_RATIO);
        // 图片等大载荷立即释放；只保留有界字形、描边槽位和命令数组分配。
        encoder.clear_commands_for_reuse(glyph_capacity_limit, stroke_capacity_limit);
        self.spare_encoder = Some(encoder);
    }

    /// 仅 flush 当前 scratch 并检查 deferred 错误（不结束录制）。
    pub(super) fn flush_recording(&mut self) -> Result<(), Error> {
        self.flush_scratch()?;
        if let Some(error) = self.deferred_error.take() {
            return Err(error);
        }
        Ok(())
    }

    /// 放弃当前录制：清理 scratch 状态并释放大块分配。
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
        self.spare_encoder = None;
        self.blend_mode = BlendMode::default();
        self.blend_stack.clear();
        self.scratch_dirty = false;
        self.scratch_additive = false;
        self.scratch_pack_bounds = None;
        self.deferred_error = None;
        self.release_scratch_allocation();
    }

    /// 把 scratch 表面缩回 1×1，释放大块分配（保留光栅化状态）。
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

    /// 保留内存估算：scratch、活动编码器与空闲命令缓冲之和。
    pub(super) fn retained_memory_usage(&self) -> usize {
        self.scratch
            .memory_usage()
            .saturating_add(
                self.encoder
                    .as_ref()
                    .map(FrameEncoder::retained_memory_usage)
                    .unwrap_or(0),
            )
            .saturating_add(
                self.spare_encoder
                    .as_ref()
                    .map(FrameEncoder::retained_memory_usage)
                    .unwrap_or(0),
            )
    }

    /// 追加一组已验证命令（先 flush 前置 scratch）。
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

    /// 录制 Picture blit：可直连时走 PictureBlit，否则经 scratch 软回退。
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

    /// 尝试把 raw image 直接录制为 PictureBlit；不可直连返回 `Ok(false)`。
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

    /// 直接录制共享图片；完整源图沿帧命令保留 `Arc`，裁剪与 Additive 保持旧复制路径。
    pub(super) fn record_direct_image_blit_shared(
        &mut self,
        pixels: std::sync::Arc<Vec<u32>>,
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
        // Additive 仍复用既有几何与错误顺序；本批只消除常见完整 SrcOver 图片复制。
        if self.blend_mode == BlendMode::Additive {
            return self.record_direct_image_blit(
                pixels.as_slice(),
                source_width,
                source_rect,
                destination_rect,
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
        // 只有完整源图能直接共享；裁剪仍必须形成紧密行主序载荷。
        if source.x != 0
            || source.y != 0
            || source.width != source_width
            || source.height != source_height
            || pixel_count != pixels.len()
        {
            return self.record_direct_image_blit(
                pixels.as_slice(),
                source_width,
                source_rect,
                destination_rect,
            );
        }
        let image = FrameImage::from_shared(source.width, source.height, pixels)
            .map_err(frame_encoder_error)?;
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

    /// 记录本地绘制区域扩展后的打包边界（并入当前 scratch 批次）。
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

    /// 把纯源贡献的软件操作录制为普通 SrcOver CPU segment 批次。
    pub(super) fn draw_cpu(
        &mut self,
        local_bounds: Rect,
        pad: f32,
        draw: impl FnOnce(&mut SharedRasterizer),
    ) {
        if self.deferred_error.is_some() {
            return;
        }
        // 透明 scratch + source-over 上传无法保留相对既有命令的 Additive
        // 语义；只有显式提升的目标相关 Native 命令才等价。
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
    /// Additive sampled segment（后者以采样纹理对累计目标饱和合成）。
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
        // 延迟到 painter-order 屏障（native op / Picture blit / finish）再
        // flush。逐操作 flush 会在每个字形和圆角填充后重新扫描上传，
        // 在密集页面上占据主要录制时间。
    }

    /// 把当前 scratch 批次编码为 CPU segment / Additive sampled 命令并清空。
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

    /// 按需分配与目标同尺寸的 scratch 表面。
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

    /// 取当前帧编码器；未在录制中则返回错误。
    pub(super) fn encoder_mut(&mut self) -> Result<&mut FrameEncoder, Error> {
        self.encoder.as_mut().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "FrameEncoder command was recorded outside its frame lifetime",
            )
        })
    }

    /// 记住首个 deferred 错误（后续错误被忽略，保留最早失败事实）。
    pub(super) fn remember_error(&mut self, error: Error) {
        if self.deferred_error.is_none() {
            self.deferred_error = Some(error);
        }
    }

    /// identity transform 下返回可直达的整数 SrcOver 矩形与矩形 clip。
    /// 记录「无法保真下放」的 deferred typed failure。
    pub(super) fn unsupported_state(&mut self, detail: &'static str) {
        self.remember_error(Error::new(
            Errc::NotImplemented,
            format!("FrameEncoder recording cannot faithfully lower {detail}"),
        ));
    }
}
