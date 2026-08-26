//! API 中立的有序帧命令模型。
//!
//! 本模块刻意与任何图形上下文解耦，提供 R1 契约基础：一条有序流涵盖
//! clear、原生工作、CPU 回退分段与 Picture/offscreen blit；唯一的表现入口
//! 消费编码器本身，因此一帧录制结果不可能被提交两次。
//!
//! 子模块划分（P2 行数治理）：`geometry` 值类型、`commands` 命令与
//! 执行契约、`error` 错误契约、`source_over` SrcOver 分组证明与裁剪、
//! `pixels` CPU 参考光栅原语；本文件只保留 [`FrameEncoder`] 主体并重导出
//! 全部公开类型，`crate::draw::painting::encoder::*` 路径保持不变。

pub(crate) mod commands;
pub(crate) mod error;
pub(crate) mod geometry;
pub(crate) mod pixels;
pub(crate) mod source_over;

use crate::draw::Color;
pub use commands::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FramePresenter, FrameRasterOp,
    GpuFrameAudit, GpuFrameViolationKind, PresentOutcome, ReferenceFrame,
};
pub use error::FrameEncoderError;
pub use geometry::{
    FrameGlyphBlit, FrameGlyphOutline, FrameImage, FrameOpacity, FrameRadius, FrameRect,
    FrameSampledRect, FrameStrokeRect, FrameStrokeWidth,
};

use self::pixels::{
    apply_raster_op_pixels, blit_image_pixels, blit_image_pixels_with_opacity,
    blit_image_pixels_with_opacity_blend, blit_sampled_image_pixels_with_opacity_blend,
    full_frame_image_blit, pixel_len,
};
use self::source_over::{
    PictureCropTranslation, crop_and_translate_source_over_command,
    source_over_commands_have_safe_grouping, stroke_batches_can_merge, stroke_visible_bounds,
};

/// 恰好一帧的有序命令录制器。
///
/// `present` 消费 `self`，这是刻意设计：一个编码器拥有且只能提交一帧，
/// 不能通过本 API 二次提交。
#[derive(Debug)]
pub struct FrameEncoder {
    width: i32,
    height: i32,
    pixel_count: usize,
    commands: Vec<FrameCommand>,
    /// 上一帧回收的一组字形槽位；仅由同一录制器跨帧复用。
    spare_glyphs: Vec<FrameGlyphBlit>,
    /// 上一帧回收的一组描边槽位；仅由同一录制器跨帧复用。
    spare_strokes: Vec<FrameStrokeRect>,
}

impl FrameEncoder {
    /// 创建指定尺寸的空帧编码器；尺寸非法或不可寻址时返回错误。
    pub fn new(width: i32, height: i32) -> Result<Self, FrameEncoderError> {
        Self::with_command_capacity(width, height, 0)
    }

    /// 创建并预留命令容量的帧编码器；用于稳定帧复用上一帧规模提示。
    pub(crate) fn with_command_capacity(
        width: i32,
        height: i32,
        command_capacity: usize,
    ) -> Result<Self, FrameEncoderError> {
        let pixel_count = pixel_len(width, height)?;
        let mut commands = Vec::new();
        commands
            .try_reserve_exact(command_capacity)
            .map_err(|_| FrameEncoderError::CommandAllocationFailed)?;
        Ok(Self {
            width,
            height,
            pixel_count,
            commands,
            spare_glyphs: Vec::new(),
            spare_strokes: Vec::new(),
        })
    }

    /// 帧宽度（像素）。
    pub const fn width(&self) -> i32 {
        self.width
    }

    /// 帧高度（像素）。
    pub const fn height(&self) -> i32 {
        self.height
    }

    /// 已录制的命令流（只读）。
    pub fn commands(&self) -> &[FrameCommand] {
        &self.commands
    }

    /// 返回命令缓冲容量，供录制器执行有界跨帧复用。
    pub(crate) fn command_capacity(&self) -> usize {
        self.commands.capacity()
    }

    /// 返回当前帧最大字形批次长度，供录制器制定有界复用策略。
    pub(crate) fn max_glyph_batch_len(&self) -> usize {
        self.commands
            .iter()
            .filter_map(|command| match command {
                FrameCommand::Native {
                    operation: FrameRasterOp::BlitGlyphs { glyphs, .. },
                } => Some(glyphs.len()),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }

    /// 返回当前帧最大描边批次长度，供录制器制定有界复用策略。
    pub(crate) fn max_stroke_batch_len(&self) -> usize {
        self.commands
            .iter()
            .filter_map(|command| match command {
                FrameCommand::Native {
                    operation: FrameRasterOp::StrokeRoundedRects { strokes, .. },
                } => Some(strokes.len()),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }

    /// 清空上一帧命令载荷，并有界保留最大字形、描边批次与命令数组分配。
    pub(crate) fn clear_commands_for_reuse(
        &mut self,
        glyph_capacity_limit: usize,
        stroke_capacity_limit: usize,
    ) {
        let mut spare_glyphs = std::mem::take(&mut self.spare_glyphs);
        if spare_glyphs.capacity() > glyph_capacity_limit {
            spare_glyphs = Vec::new();
        }
        let mut spare_strokes = std::mem::take(&mut self.spare_strokes);
        if spare_strokes.capacity() > stroke_capacity_limit {
            spare_strokes = Vec::new();
        }
        for command in &mut self.commands {
            match command {
                FrameCommand::Native {
                    operation: FrameRasterOp::BlitGlyphs { glyphs, .. },
                } if glyphs.capacity() <= glyph_capacity_limit
                    && glyphs.capacity() > spare_glyphs.capacity() =>
                {
                    std::mem::swap(glyphs, &mut spare_glyphs);
                }
                FrameCommand::Native {
                    operation: FrameRasterOp::StrokeRoundedRects { strokes, .. },
                } if strokes.capacity() <= stroke_capacity_limit
                    && strokes.capacity() > spare_strokes.capacity() =>
                {
                    std::mem::swap(strokes, &mut spare_strokes);
                }
                _ => {}
            }
        }
        self.commands.clear();
        spare_glyphs.clear();
        spare_strokes.clear();
        self.spare_glyphs = spare_glyphs;
        self.spare_strokes = spare_strokes;
    }

    /// 统计帧内 CPU 生成的光栅载荷（字形 coverage、CPU 分段、物化 Picture），
    /// 不分配也不执行帧。
    pub fn gpu_native_audit(&self) -> GpuFrameAudit {
        let mut audit = GpuFrameAudit::default();
        for command in &self.commands {
            match command {
                FrameCommand::Native {
                    operation: FrameRasterOp::BlitGlyphs { glyphs, .. },
                } => {
                    audit.cpu_glyphs = audit.cpu_glyphs.saturating_add(glyphs.len());
                    audit.cpu_glyph_bytes = audit.cpu_glyph_bytes.saturating_add(
                        glyphs
                            .iter()
                            .map(|glyph| glyph.coverage.len())
                            .fold(0usize, usize::saturating_add),
                    );
                }
                FrameCommand::CpuSegment { image, .. } => {
                    audit.cpu_raster_segments = audit.cpu_raster_segments.saturating_add(1);
                    audit.cpu_raster_bytes = audit.cpu_raster_bytes.saturating_add(
                        image
                            .pixels
                            .len()
                            .saturating_mul(std::mem::size_of::<u32>()),
                    );
                }
                FrameCommand::PictureBlit { image, .. } => {
                    audit.materialized_pictures = audit.materialized_pictures.saturating_add(1);
                    audit.materialized_picture_bytes =
                        audit.materialized_picture_bytes.saturating_add(
                            image
                                .pixels
                                .len()
                                .saturating_mul(std::mem::size_of::<u32>()),
                        );
                }
                FrameCommand::Clear { .. } | FrameCommand::Native { .. } => {}
            }
        }
        audit
    }

    /// 在引擎边界强制 GPU-only 提交契约：存在 CPU 载荷违规时返回带明细的错误。
    pub fn validate_gpu_native(&self) -> Result<(), FrameEncoderError> {
        let audit = self.gpu_native_audit();
        let Some(kind) = audit.first_violation() else {
            return Ok(());
        };
        let payload_bytes = match kind {
            GpuFrameViolationKind::CpuRasterSegment => audit.cpu_raster_bytes,
            GpuFrameViolationKind::CpuGlyphCoverage => audit.cpu_glyph_bytes,
            GpuFrameViolationKind::MaterializedPicture => audit.materialized_picture_bytes,
        };
        Err(FrameEncoderError::GpuNativeViolation {
            kind,
            payload_bytes,
        })
    }

    /// 保守的保留载荷大小估算，用于让录制的 Picture 不超过其此前完整 BGRA
    /// surface 的体积。共享分配可能被重复计数；宁可高估，从而选择有界的
    /// 物化回退而不是保留无界的命令载荷。
    pub(crate) fn retained_memory_usage(&self) -> usize {
        let mut bytes = self
            .commands
            .capacity()
            .saturating_mul(std::mem::size_of::<FrameCommand>())
            .saturating_add(
                self.spare_glyphs
                    .capacity()
                    .saturating_mul(std::mem::size_of::<FrameGlyphBlit>()),
            )
            .saturating_add(
                self.spare_strokes
                    .capacity()
                    .saturating_mul(std::mem::size_of::<FrameStrokeRect>()),
            );
        for command in &self.commands {
            bytes = bytes.saturating_add(match command {
                FrameCommand::Native {
                    operation: FrameRasterOp::BlitGlyphs { glyphs, .. },
                } => glyphs
                    .capacity()
                    .saturating_mul(std::mem::size_of::<FrameGlyphBlit>())
                    .saturating_add(
                        glyphs
                            .iter()
                            .map(|glyph| glyph.coverage.len())
                            .fold(0usize, usize::saturating_add),
                    ),
                FrameCommand::Native {
                    operation: FrameRasterOp::BlitGlyphOutlines { glyphs, .. },
                } => glyphs
                    // 保守计入轮廓命令数组容量。
                    .capacity()
                    .saturating_mul(std::mem::size_of::<FrameGlyphOutline>())
                    // 保守计入每个共享边列表载荷。
                    .saturating_add(
                        glyphs
                            .iter()
                            .map(|glyph| {
                                glyph
                                    .edges()
                                    .len()
                                    .saturating_mul(std::mem::size_of::<f32>())
                            })
                            .fold(0usize, usize::saturating_add),
                    ),
                FrameCommand::Native {
                    operation: FrameRasterOp::StrokeRoundedRects { strokes, .. },
                } => strokes
                    .capacity()
                    .saturating_mul(std::mem::size_of::<FrameStrokeRect>()),
                FrameCommand::CpuSegment { image, .. }
                | FrameCommand::PictureBlit { image, .. } => image
                    .pixels
                    .len()
                    .saturating_mul(std::mem::size_of::<u32>()),
                FrameCommand::Clear { .. } | FrameCommand::Native { .. } => 0,
            });
        }
        bytes
    }

    /// 生成透明 SrcOver-only 命令子集的平移副本。这是代数上安全的 Picture
    /// splice：写入要么互不相交，要么每个量化重叠都完全由已证明不透明的
    /// 无间隙区域并集支撑，因此透明中间合成与直接在父流中执行这些命令等价。
    ///
    /// 验证是原子的：不支持的 clear、目标相关操作、越界载荷或坐标溢出都会
    /// 在父编码器被改动前返回 `None`。
    pub(crate) fn translated_source_over_commands(
        &self,
        dx: i32,
        dy: i32,
        target_width: i32,
        target_height: i32,
    ) -> Option<Vec<FrameCommand>> {
        self.translated_source_over_commands_in(
            FrameRect::new(0, 0, self.width, self.height),
            dx,
            dy,
            target_width,
            target_height,
        )
    }

    /// 生成透明 SrcOver-only 命令子集的整数平移、1:1 裁剪副本。跨越
    /// `source` 的几何保持原形状并附加精确整数 clip；完全不可见的命令被
    /// 省略。图片命令只保留对应的源子矩形。
    ///
    /// 与整幅 Picture 形态一样，本方法在调用方可追加任何内容前先构建完整
    /// 的临时命令列表。
    pub(crate) fn translated_source_over_commands_in(
        &self,
        source: FrameRect,
        dx: i32,
        dy: i32,
        target_width: i32,
        target_height: i32,
    ) -> Option<Vec<FrameCommand>> {
        if !source.is_within(self.width, self.height) {
            return None;
        }
        let (first, commands) = self.commands.split_first()?;
        if !matches!(first, FrameCommand::Clear { color } if color.premultiplied() == 0) {
            return None;
        }
        if !source_over_commands_have_safe_grouping(commands, self.width, self.height) {
            return None;
        }
        let translation = PictureCropTranslation {
            source_width: self.width,
            source_height: self.height,
            source_crop: source,
            dx,
            dy,
            target_width,
            target_height,
        };
        let mut translated = Vec::new();
        translated.try_reserve_exact(commands.len()).ok()?;
        for command in commands {
            match crop_and_translate_source_over_command(command, &translation) {
                Ok(Some(command)) => translated.push(command),
                Ok(None) => {}
                Err(()) => return None,
            }
        }
        Some(translated)
    }

    /// 追加一组已验证命令；预留失败返回命令分配错误。
    pub(crate) fn append_validated_commands(
        &mut self,
        commands: Vec<FrameCommand>,
    ) -> Result<(), FrameEncoderError> {
        self.commands
            .try_reserve(commands.len())
            .map_err(|_| FrameEncoderError::CommandAllocationFailed)?;
        self.commands.extend(commands);
        Ok(())
    }

    /// 记录整帧清除命令。
    pub fn clear(&mut self, color: Color) {
        self.commands.push(FrameCommand::Clear { color });
    }

    /// 记录一条原生光栅命令；同 clip 的连续字形 / 可安全合并的描边会被批合并。
    pub fn native(&mut self, operation: FrameRasterOp) {
        if let FrameRasterOp::BlitGlyphs { glyphs, clip } = operation {
            if glyphs.is_empty() || clip.is_empty() {
                return;
            }
            if let Some(FrameCommand::Native {
                operation:
                    FrameRasterOp::BlitGlyphs {
                        glyphs: previous,
                        clip: previous_clip,
                    },
            }) = self.commands.last_mut()
            {
                if *previous_clip == clip {
                    previous.extend(glyphs);
                    return;
                }
            }
            self.commands.push(FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { glyphs, clip },
            });
            return;
        }
        // 描边批次同时保留 clip 与 blend 事实，防止 SrcOver/Additive 越界合并。
        if let FrameRasterOp::StrokeRoundedRects {
            strokes,
            clip,
            additive,
        } = operation
        {
            for stroke in strokes {
                self.native_stroke(stroke, clip, additive);
            }
            return;
        }
        self.commands.push(FrameCommand::Native { operation });
    }

    /// 直接追加单条描边，避免为单元素输入和新批次各构造一次临时 `Vec`。
    pub(crate) fn native_stroke(
        &mut self,
        stroke: FrameStrokeRect,
        clip: FrameRect,
        additive: bool,
    ) {
        if clip.is_empty() || stroke_visible_bounds(stroke, clip, self.width, self.height).is_none()
        {
            return;
        }
        if let Some(FrameCommand::Native {
            operation:
                FrameRasterOp::StrokeRoundedRects {
                    strokes: previous,
                    clip: previous_clip,
                    additive: previous_additive,
                },
        }) = self.commands.last_mut()
        {
            if *previous_clip == clip
                && *previous_additive == additive
                && stroke_batches_can_merge(
                    previous,
                    std::slice::from_ref(&stroke),
                    clip,
                    self.width,
                    self.height,
                )
            {
                previous.push(stroke);
                return;
            }
        }
        let mut strokes = std::mem::take(&mut self.spare_strokes);
        strokes.clear();
        strokes.push(stroke);
        self.commands.push(FrameCommand::Native {
            operation: FrameRasterOp::StrokeRoundedRects {
                strokes,
                clip,
                // 新批次继承调用方已经证明的 blend 事实。
                additive,
            },
        });
    }

    /// 直接追加单个字形，避免为每个 glyph 构造一次临时 `Vec`。
    pub(crate) fn native_glyph(&mut self, glyph: FrameGlyphBlit, clip: FrameRect) {
        if clip.is_empty() {
            return;
        }
        if let Some(FrameCommand::Native {
            operation:
                FrameRasterOp::BlitGlyphs {
                    glyphs,
                    clip: previous_clip,
                },
        }) = self.commands.last_mut()
        {
            if *previous_clip == clip {
                glyphs.push(glyph);
                return;
            }
        }
        let mut glyphs = std::mem::take(&mut self.spare_glyphs);
        glyphs.push(glyph);
        self.commands.push(FrameCommand::Native {
            operation: FrameRasterOp::BlitGlyphs { glyphs, clip },
        });
    }

    /// 记录与源无关的 CPU 光栅子集，作为一个有界回退分段。目标相关操作
    /// 必须对累计目标执行，会在本编码器被改动前被拒绝。
    pub fn cpu_segment(
        &mut self,
        operations: impl IntoIterator<Item = FrameRasterOp>,
    ) -> Result<(), FrameEncoderError> {
        let operations = operations.into_iter().collect::<Vec<_>>();
        if operations.is_empty() {
            return Ok(());
        }
        if let Some(operation) = operations.iter().find_map(|operation| match operation {
            FrameRasterOp::FillRect { .. }
            | FrameRasterOp::FillRoundedRect { .. }
            | FrameRasterOp::FillRoundedRectClipped { .. }
            | FrameRasterOp::FillRoundedRectSubpixel { .. }
            | FrameRasterOp::BlitGlyphs { .. }
            | FrameRasterOp::BlitGlyphOutlines { .. }
            | FrameRasterOp::StrokeRoundedRects {
                additive: false, ..
            }
            | FrameRasterOp::StrokeRoundedRectSubpixel { .. } => None,
            // Additive 描边必须读取累计目标，不能先画进透明分段再 SrcOver 合成。
            FrameRasterOp::StrokeRoundedRects { additive: true, .. } => {
                Some("StrokeRoundedRectsAdditive")
            }
            FrameRasterOp::FillRectAdditive { .. } => Some("FillRectAdditive"),
            FrameRasterOp::FillRoundedRectAdditive { .. } => Some("FillRoundedRectAdditive"),
            FrameRasterOp::ScrollCopy { .. } => Some("ScrollCopy"),
        }) {
            return Err(FrameEncoderError::DestinationDependentCpuSegment { operation });
        }
        let mut image = self.transparent_reference();
        for operation in &operations {
            apply_raster_op_pixels(self.width, self.height, &mut image.pixels, operation);
        }
        let full = FrameRect::new(0, 0, self.width, self.height);
        self.commands.push(FrameCommand::CpuSegment {
            image: FrameImage {
                width: image.width,
                height: image.height,
                pixels: image.pixels.into(),
            },
            src: full,
            dst: full,
        });
        Ok(())
    }

    /// 记录一段精确 CPU 光栅化的源分段。载荷是不可变的 API 中立像素；
    /// 目标是命令的一部分，而不是隐式的全帧载体。
    pub fn cpu_image_segment(&mut self, image: FrameImage, src: FrameRect, dst: FrameRect) {
        if src.is_empty() || dst.is_empty() {
            return;
        }
        self.commands
            .push(FrameCommand::CpuSegment { image, src, dst });
    }

    /// 记录整像素、不透明 SrcOver 的 Picture blit。
    pub fn blit_picture(&mut self, image: FrameImage, src: FrameRect, dst: FrameRect) {
        self.blit_picture_with_opacity(
            image,
            src,
            FrameSampledRect::from_integer(dst),
            FrameOpacity::opaque(),
        );
    }

    /// 记录带 opacity 的 Picture blit（整数目标）。
    pub(crate) fn blit_picture_with_opacity(
        &mut self,
        image: FrameImage,
        src: FrameRect,
        dst: FrameSampledRect,
        opacity: FrameOpacity,
    ) {
        self.blit_picture_with_opacity_blend(image, src, dst, opacity, false);
    }

    /// 记录带 opacity 与 Additive 事实的 Picture blit；空源 / 空目标 /
    /// 全透明 opacity 是安全 no-op。
    pub(crate) fn blit_picture_with_opacity_blend(
        &mut self,
        image: FrameImage,
        src: FrameRect,
        dst: FrameSampledRect,
        opacity: FrameOpacity,
        additive: bool,
    ) {
        // 页面销毁阶段的空图片或透明图片与 CPU 参考执行一致，属于安全 no-op。
        if src.is_empty() || dst.is_empty() || opacity.is_transparent() {
            // 不把无像素贡献的命令交给 retained RHI lowering。
            return;
        }
        self.commands.push(FrameCommand::PictureBlit {
            image,
            src,
            dst,
            opacity,
            additive,
        });
    }

    /// 整数目标兼容入口；fractional / 缩放目标请用 [`Self::blit_picture_with_opacity`]。
    pub(crate) fn blit_picture_integer_with_opacity(
        &mut self,
        image: FrameImage,
        src: FrameRect,
        dst: FrameRect,
        opacity: FrameOpacity,
    ) {
        self.blit_picture_with_opacity(image, src, FrameSampledRect::from_integer(dst), opacity);
    }

    /// 在内存中执行这一刻意精简的参考子集。
    ///
    /// 生产 API 渲染器必须保持该命令顺序；它们不把本执行器当作自己的
    /// 渲染实现。
    pub fn render_reference(&self) -> ReferenceFrame {
        let mut frame = self.transparent_reference();
        self.execute_into_pixels(&mut frame.pixels);
        frame
    }

    /// 把整帧渲染为可复用的 `FrameImage`（参考路径）。
    pub(crate) fn render_image(&self) -> FrameImage {
        let frame = self.render_reference();
        FrameImage {
            width: frame.width,
            height: frame.height,
            pixels: frame.pixels.into(),
        }
    }

    /// 把有序命令流执行到本编码器尺寸的 CPU 目标中。CPU 后端使用它；
    /// API 原生后端在自己的边界消费同样的 [`FrameCommand`] 变体。
    ///
    /// **不**会在命令前擦除目标：全帧以 [`FrameCommand::Clear`] 开头；
    /// 脏帧依赖 `begin_frame(DirtyRects)` 只清除过 damage AABB，
    /// 未损坏像素得以保留。
    pub(crate) fn execute_into_pixels(&self, pixels: &mut [u32]) {
        assert_eq!(
            pixels.len(),
            self.pixel_count,
            "FrameEncoder target must match its recorded extent"
        );
        let mut target_is_transparent = false;
        for command in &self.commands {
            match command {
                // Clear 是替换操作，永不是透明 source-over。
                FrameCommand::Clear { color } => {
                    let clear = color.premultiplied();
                    pixels.fill(clear);
                    target_is_transparent = clear == 0;
                }
                FrameCommand::Native { operation } => {
                    apply_raster_op_pixels(self.width, self.height, pixels, operation);
                    target_is_transparent = false;
                }
                FrameCommand::CpuSegment { image, src, dst } => {
                    blit_image_pixels(self.width, self.height, pixels, image, *src, *dst);
                    target_is_transparent = false;
                }
                FrameCommand::PictureBlit {
                    image,
                    src,
                    dst,
                    opacity,
                    additive,
                } => {
                    if opacity.is_transparent() {
                        continue;
                    }
                    if let Some(integer_dst) = dst.as_integer() {
                        // 透明目标上的 source-over 恰好等于预乘源本身。保留
                        // 背景恢复正利用这一有序形态，避免每个全表面像素都
                        // 多一次 alpha 分支与混合决策。
                        if !*additive
                            && target_is_transparent
                            && opacity.is_opaque()
                            && full_frame_image_blit(
                                self.width,
                                self.height,
                                image,
                                *src,
                                integer_dst,
                            )
                        {
                            pixels.copy_from_slice(image.pixels());
                        } else {
                            blit_image_pixels_with_opacity_blend(
                                self.width,
                                self.height,
                                pixels,
                                image,
                                *src,
                                integer_dst,
                                opacity.value(),
                                *additive,
                            );
                        }
                    } else {
                        // 亚像素 / 缩放：走浮点采样 blit，与 GPU 纹理四边形语义对齐。
                        blit_sampled_image_pixels_with_opacity_blend(
                            self.width,
                            self.height,
                            pixels,
                            image,
                            *src,
                            *dst,
                            opacity.value(),
                            *additive,
                        );
                    }
                    target_is_transparent = false;
                }
            }
        }
    }

    /// 把一段图片分段直接光栅化到其可见目标 tile。原生执行器在录制点按
    /// alpha 混合这段精确分段，无需分配或扫描透明全帧大小的源。
    // CPU 分段参考 tile 是跨后端兼容诊断入口，当前测试矩阵按需调用。
    #[allow(dead_code)]
    pub(crate) fn cpu_segment_reference_tile(
        &self,
        image: &FrameImage,
        src: FrameRect,
        dst: FrameRect,
    ) -> Option<(ReferenceFrame, FrameRect)> {
        self.image_blit_reference_tile(image, src, dst, 1.0)
    }

    /// 把一次图片 blit 直接光栅化到可见目标 tile（参考 / 诊断共用实现）。
    fn image_blit_reference_tile(
        &self,
        image: &FrameImage,
        src: FrameRect,
        dst: FrameRect,
        opacity: f32,
    ) -> Option<(ReferenceFrame, FrameRect)> {
        let visible = dst.intersection(FrameRect::new(0, 0, self.width, self.height))?;
        let pixel_count =
            usize::try_from(i64::from(visible.width).checked_mul(i64::from(visible.height))?)
                .ok()?;
        let local_dst = FrameRect::new(
            dst.x.checked_sub(visible.x)?,
            dst.y.checked_sub(visible.y)?,
            dst.width,
            dst.height,
        );
        let mut frame = ReferenceFrame {
            width: visible.width,
            height: visible.height,
            pixels: vec![Color::transparent().premultiplied(); pixel_count],
        };
        blit_image_pixels_with_opacity(
            visible.width,
            visible.height,
            &mut frame.pixels,
            image,
            src,
            local_dst,
            opacity,
        );
        Some((frame, visible))
    }

    /// 创建全透明参考帧（预乘透明像素）。
    fn transparent_reference(&self) -> ReferenceFrame {
        ReferenceFrame {
            width: self.width,
            height: self.height,
            pixels: vec![Color::transparent().premultiplied(); self.pixel_count],
        }
    }

    /// 最终提交会消费编码器，因此公开命令模型每帧只有一个最终 present 操作。
    pub fn present<P: FramePresenter>(self, presenter: &mut P) -> Result<PresentOutcome, P::Error> {
        presenter.present(&self.render_reference())
    }
}
