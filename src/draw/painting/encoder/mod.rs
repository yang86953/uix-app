//! API-neutral ordered frame command model.
//!
//! This is deliberately independent from any graphics context.  It provides
//! the R1 contract foundation: one ordered stream for clear, native work, CPU
//! fallback segments, and Picture/offscreen blits; the only presentation entry
//! consumes the encoder, so a recorded frame cannot be presented twice.
//!
//! 子模块划分（P2 行数治理）：[`geometry`] 值类型、[`commands`] 命令与
//! 执行契约、[`error`] 错误契约、[`source_over`] SrcOver 分组证明与裁剪、
//! [`pixels`] CPU 参考光栅原语；本文件只保留 [`FrameEncoder`] 主体并重导出
//! 全部公开类型，`crate::draw::painting::encoder::*` 路径保持不变。

pub(crate) mod commands;
pub(crate) mod error;
pub(crate) mod geometry;
pub(crate) mod pixels;
pub(crate) mod source_over;

pub use commands::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FramePresenter, FrameRasterOp,
    GpuFrameAudit, GpuFrameViolationKind, PresentOutcome, ReferenceFrame,
};
pub use error::FrameEncoderError;
pub use geometry::{
    FrameGlyphBlit, FrameImage, FrameOpacity, FrameRadius, FrameRect, FrameSampledRect,
    FrameStrokeRect, FrameStrokeWidth,
};
pub(crate) use pixels::apply_frame_raster_op;

use crate::draw::Color;

use self::pixels::{
    apply_raster_op_pixels, apply_stroke_rect_pixels, blit_image_pixels,
    blit_image_pixels_with_opacity, blit_image_pixels_with_opacity_blend,
    blit_sampled_image_pixels_with_opacity, blit_sampled_image_pixels_with_opacity_blend,
    full_frame_image_blit, pixel_len,
};
use self::source_over::{
    crop_and_translate_source_over_command, source_over_commands_have_safe_grouping,
    stroke_batch_bounds, stroke_batches_can_merge, stroke_visible_bounds, PictureCropTranslation,
};


/// Ordered command recorder for exactly one frame.
///
/// `present` consumes `self`.  This is intentional: an encoder owns one frame
/// and cannot be submitted a second time through this API.
#[derive(Debug)]
pub struct FrameEncoder {
    width: i32,
    height: i32,
    pixel_count: usize,
    commands: Vec<FrameCommand>,
}

impl FrameEncoder {
    pub fn new(width: i32, height: i32) -> Result<Self, FrameEncoderError> {
        let pixel_count = pixel_len(width, height)?;
        Ok(Self {
            width,
            height,
            pixel_count,
            commands: Vec::new(),
        })
    }

    pub const fn width(&self) -> i32 {
        self.width
    }

    pub const fn height(&self) -> i32 {
        self.height
    }

    pub fn commands(&self) -> &[FrameCommand] {
        &self.commands
    }

    /// Reports whether the frame contains any CPU-generated raster payload.
    /// This deliberately does not allocate or execute the frame.
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

    /// Enforces the GPU-only submission contract at the engine boundary.
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

    /// Conservative retained payload size used to keep a recorded Picture no
    /// larger than its former full BGRA surface. Shared allocations may be
    /// counted more than once; over-counting deliberately selects the bounded
    /// materialized fallback instead of retaining unbounded command payloads.
    pub(crate) fn retained_memory_usage(&self) -> usize {
        let mut bytes = self
            .commands
            .capacity()
            .saturating_mul(std::mem::size_of::<FrameCommand>());
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

    /// Produces a translated copy of the transparent SrcOver-only command
    /// subset. This is the algebraically safe Picture splice: writes are either
    /// disjoint or every quantizing overlap is fully backed by a gap-free union
    /// of proven-opaque regions, so transparent-intermediate composition remains
    /// equivalent to issuing the commands directly in the parent stream.
    ///
    /// Validation is atomic. Unsupported clears, destination-dependent ops,
    /// out-of-bounds payloads, or coordinate overflow return `None` before the
    /// parent encoder is mutated.
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

    /// Produces an integer-translated, 1:1 crop of the transparent
    /// SrcOver-only command subset. Geometry that crosses `source` keeps its
    /// original shape and gains an exact integer clip; fully invisible
    /// commands are omitted. Image commands retain only the corresponding
    /// source sub-rectangle.
    ///
    /// Like the full-Picture form above, this method builds a complete
    /// temporary command list before the caller can append anything.
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

    pub fn clear(&mut self, color: Color) {
        self.commands.push(FrameCommand::Clear { color });
    }

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
        if let FrameRasterOp::StrokeRoundedRects { strokes, clip } = operation {
            if clip.is_empty() {
                return;
            }
            for stroke in strokes {
                if stroke_visible_bounds(stroke, clip, self.width, self.height).is_none() {
                    continue;
                }
                if let Some(FrameCommand::Native {
                    operation:
                        FrameRasterOp::StrokeRoundedRects {
                            strokes: previous,
                            clip: previous_clip,
                        },
                }) = self.commands.last_mut()
                {
                    if *previous_clip == clip
                        && stroke_batches_can_merge(
                            previous,
                            std::slice::from_ref(&stroke),
                            clip,
                            self.width,
                            self.height,
                        )
                    {
                        previous.push(stroke);
                        continue;
                    }
                }
                self.commands.push(FrameCommand::Native {
                    operation: FrameRasterOp::StrokeRoundedRects {
                        strokes: vec![stroke],
                        clip,
                    },
                });
            }
            return;
        }
        self.commands.push(FrameCommand::Native { operation });
    }

    /// Records the source-independent CPU-raster subset as one bounded fallback
    /// segment. Destination-dependent operations must execute against the
    /// accumulating target and are rejected before this encoder is mutated.
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
            | FrameRasterOp::BlitGlyphs { .. }
            | FrameRasterOp::StrokeRoundedRects { .. } => None,
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

    /// Records an exact CPU-rasterized source segment. The payload is
    /// immutable, API-neutral pixels; the destination is part of the command
    /// rather than an implicit full-frame carrier.
    pub fn cpu_image_segment(&mut self, image: FrameImage, src: FrameRect, dst: FrameRect) {
        if src.is_empty() || dst.is_empty() {
            return;
        }
        self.commands
            .push(FrameCommand::CpuSegment { image, src, dst });
    }

    pub fn blit_picture(&mut self, image: FrameImage, src: FrameRect, dst: FrameRect) {
        self.blit_picture_with_opacity(
            image,
            src,
            FrameSampledRect::from_integer(dst),
            FrameOpacity::opaque(),
        );
    }

    pub(crate) fn blit_picture_with_opacity(
        &mut self,
        image: FrameImage,
        src: FrameRect,
        dst: FrameSampledRect,
        opacity: FrameOpacity,
    ) {
        self.blit_picture_with_opacity_blend(image, src, dst, opacity, false);
    }

    pub(crate) fn blit_picture_with_opacity_blend(
        &mut self,
        image: FrameImage,
        src: FrameRect,
        dst: FrameSampledRect,
        opacity: FrameOpacity,
        additive: bool,
    ) {
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

    /// Executes this deliberately small reference subset in memory.
    ///
    /// Production API renderers must preserve this command order; they do not
    /// use this executor as their rendering implementation.
    pub fn render_reference(&self) -> ReferenceFrame {
        let mut frame = self.transparent_reference();
        self.execute_into_pixels(&mut frame.pixels);
        frame
    }

    pub(crate) fn render_image(&self) -> FrameImage {
        let frame = self.render_reference();
        FrameImage {
            width: frame.width,
            height: frame.height,
            pixels: frame.pixels.into(),
        }
    }

    /// Executes the ordered command stream into a CPU target of this
    /// encoder's extent. This is used by the CPU backend; API-native backends
    /// consume the same [`FrameCommand`] variants at their own boundaries.
    ///
    /// Does **not** wipe the target before commands: full frames start with
    /// [`FrameCommand::Clear`]; dirty frames rely on `begin_frame(DirtyRects)`
    /// having cleared only the damage AABB so undamaged pixels stay retained.
    pub(crate) fn execute_into_pixels(&self, pixels: &mut [u32]) {
        assert_eq!(
            pixels.len(),
            self.pixel_count,
            "FrameEncoder target must match its recorded extent"
        );
        let mut target_is_transparent = false;
        for command in &self.commands {
            match command {
                // Clear is a replace operation, never transparent source-over.
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
                            // Source-over onto a transparent target is exactly the
                            // premultiplied source. Retained backdrop restores use
                            // this ordered shape, avoiding one alpha branch and
                            // blend decision per full-surface pixel.
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

    /// Rasterizes one image segment directly into its visible destination
    /// tile. Native executors alpha-blit this exact segment at its recorded
    /// point without allocating or scanning a transparent frame-sized source.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn cpu_segment_reference_tile(
        &self,
        image: &FrameImage,
        src: FrameRect,
        dst: FrameRect,
    ) -> Option<(ReferenceFrame, FrameRect)> {
        self.image_blit_reference_tile(image, src, dst, 1.0)
    }

    /// Rasterizes one Picture blit directly into its visible destination tile
    /// for API-native execution at its exact painter-order boundary.
    pub(crate) fn picture_blit_reference_tile(
        &self,
        image: &FrameImage,
        src: FrameRect,
        dst: FrameSampledRect,
        opacity: FrameOpacity,
    ) -> Option<(ReferenceFrame, FrameRect)> {
        if opacity.is_transparent() {
            return None;
        }
        let Some(integer_dst) = dst.as_integer() else {
            // 亚像素目标：覆盖整数 AABB 内做浮点采样参考。
            return self.sampled_picture_blit_reference_tile(image, src, dst, opacity.value());
        };
        self.image_blit_reference_tile(image, src, integer_dst, opacity.value())
    }

    fn sampled_picture_blit_reference_tile(
        &self,
        image: &FrameImage,
        src: FrameRect,
        dst: FrameSampledRect,
        opacity: f32,
    ) -> Option<(ReferenceFrame, FrameRect)> {
        let x0 = dst.x().floor().max(0.0) as i32;
        let y0 = dst.y().floor().max(0.0) as i32;
        let x1 = (dst.x() + dst.width()).ceil().min(self.width as f32) as i32;
        let y1 = (dst.y() + dst.height()).ceil().min(self.height as f32) as i32;
        if x0 >= x1 || y0 >= y1 {
            return None;
        }
        let visible = FrameRect::new(x0, y0, x1 - x0, y1 - y0);
        let pixel_count =
            usize::try_from(i64::from(visible.width).checked_mul(i64::from(visible.height))?)
                .ok()?;
        let mut frame = ReferenceFrame {
            width: visible.width,
            height: visible.height,
            pixels: vec![Color::transparent().premultiplied(); pixel_count],
        };
        let local_dst = FrameSampledRect::from_parts(
            dst.x() - visible.x as f32,
            dst.y() - visible.y as f32,
            dst.width(),
            dst.height(),
        )
        .ok()?;
        blit_sampled_image_pixels_with_opacity(
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

    /// Rasterizes one retained rectangle stroke into its conservative visible
    /// tile for backends without native stroke-rect capability.
    pub(crate) fn stroke_rects_reference_tile(
        &self,
        strokes: &[FrameStrokeRect],
        clip: FrameRect,
    ) -> Result<Option<(ReferenceFrame, FrameRect)>, FrameEncoderError> {
        let Some((visible, _)) = stroke_batch_bounds(strokes, clip, self.width, self.height) else {
            return Ok(None);
        };
        let pixel_count = pixel_len(visible.width, visible.height)?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(pixel_count)
            .map_err(|_| FrameEncoderError::CommandAllocationFailed)?;
        pixels.resize(pixel_count, Color::transparent().premultiplied());
        let local_clip = FrameRect::new(0, 0, visible.width, visible.height);
        for stroke in strokes {
            let local_rect =
                FrameRect::new(
                    stroke.rect.x.checked_sub(visible.x).ok_or(
                        FrameEncoderError::InvalidExtent {
                            width: self.width,
                            height: self.height,
                        },
                    )?,
                    stroke.rect.y.checked_sub(visible.y).ok_or(
                        FrameEncoderError::InvalidExtent {
                            width: self.width,
                            height: self.height,
                        },
                    )?,
                    stroke.rect.width,
                    stroke.rect.height,
                );
            apply_stroke_rect_pixels(
                visible.width,
                visible.height,
                &mut pixels,
                FrameStrokeRect::new(local_rect, stroke.color, stroke.radius, stroke.line_width),
                local_clip,
            );
        }
        Ok(Some((
            ReferenceFrame {
                width: visible.width,
                height: visible.height,
                pixels,
            },
            visible,
        )))
    }

    fn transparent_reference(&self) -> ReferenceFrame {
        ReferenceFrame {
            width: self.width,
            height: self.height,
            pixels: vec![Color::transparent().premultiplied(); self.pixel_count],
        }
    }

    /// Final submission consumes the encoder so the public command model has
    /// one final present operation per frame.
    pub fn present<P: FramePresenter>(self, presenter: &mut P) -> Result<PresentOutcome, P::Error> {
        presenter.present(&self.render_reference())
    }
}
