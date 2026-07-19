//! API-neutral ordered frame command model.
//!
//! This is deliberately independent from any graphics context.  It provides
//! the R1 contract foundation: one ordered stream for clear, native work, CPU
//! fallback segments, and Picture/offscreen blits; the only presentation entry
//! consumes the encoder, so a recorded frame cannot be presented twice.

use crate::core::Rect;
use crate::draw::engine::cpu::raster_renderer::RasterRenderer;
use crate::draw::primitives::types::{BlendMode, Radius};
use crate::draw::Color;
use std::sync::Arc;

/// Integer pixel rectangle used by the API-neutral frame command model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl FrameRect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    pub(crate) fn intersection(self, other: Self) -> Option<Self> {
        let left = i64::from(self.x).max(i64::from(other.x));
        let top = i64::from(self.y).max(i64::from(other.y));
        let right = (i64::from(self.x) + i64::from(self.width))
            .min(i64::from(other.x) + i64::from(other.width));
        let bottom = (i64::from(self.y) + i64::from(self.height))
            .min(i64::from(other.y) + i64::from(other.height));
        if left >= right || top >= bottom {
            return None;
        }
        Some(Self::new(
            left as i32,
            top as i32,
            (right - left) as i32,
            (bottom - top) as i32,
        ))
    }

    fn translated(self, dx: i32, dy: i32) -> Option<Self> {
        Some(Self::new(
            self.x.checked_add(dx)?,
            self.y.checked_add(dy)?,
            self.width,
            self.height,
        ))
    }

    pub(crate) fn is_within(self, width: i32, height: i32) -> bool {
        if self.is_empty() || self.x < 0 || self.y < 0 {
            return false;
        }
        i64::from(self.x) + i64::from(self.width) <= i64::from(width)
            && i64::from(self.y) + i64::from(self.height) <= i64::from(height)
    }
}

/// 帧命令使用的已验证圆角半径。
///
/// 构造时排除 NaN、无穷大与负值，并把 `-0.0` 规范化为 `0.0`，因此该类型
/// 可以安全保持命令模型原有的 `Eq` 契约。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameRadius {
    value: Radius,
}

impl FrameRadius {
    pub(crate) const fn zero() -> Self {
        Self {
            value: Radius {
                tl: 0.0,
                tr: 0.0,
                br: 0.0,
                bl: 0.0,
            },
        }
    }

    pub fn new(value: Radius) -> Result<Self, FrameEncoderError> {
        for (corner, radius) in [
            ("top-left", value.tl),
            ("top-right", value.tr),
            ("bottom-right", value.br),
            ("bottom-left", value.bl),
        ] {
            if !radius.is_finite() || radius < 0.0 {
                return Err(FrameEncoderError::InvalidRadius { corner });
            }
        }
        let canonical = |radius: f32| if radius == 0.0 { 0.0 } else { radius };
        Ok(Self {
            value: Radius {
                tl: canonical(value.tl),
                tr: canonical(value.tr),
                br: canonical(value.br),
                bl: canonical(value.bl),
            },
        })
    }

    pub const fn to_radius(self) -> Radius {
        self.value
    }
}

impl Eq for FrameRadius {}

/// Validated finite positive stroke width retained by frame commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameStrokeWidth(u32);

impl FrameStrokeWidth {
    pub fn new(value: f32) -> Result<Self, FrameEncoderError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(FrameEncoderError::InvalidStrokeWidth);
        }
        Ok(Self(value.to_bits()))
    }

    pub const fn value(self) -> f32 {
        f32::from_bits(self.0)
    }
}

/// One validated rectangle stroke retained by a batched frame operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameStrokeRect {
    rect: FrameRect,
    color: Color,
    radius: FrameRadius,
    line_width: FrameStrokeWidth,
}

impl FrameStrokeRect {
    pub const fn new(
        rect: FrameRect,
        color: Color,
        radius: FrameRadius,
        line_width: FrameStrokeWidth,
    ) -> Self {
        Self {
            rect,
            color,
            radius,
            line_width,
        }
    }

    pub const fn rect(&self) -> FrameRect {
        self.rect
    }

    pub const fn color(&self) -> Color {
        self.color
    }

    pub const fn radius(&self) -> FrameRadius {
        self.radius
    }

    pub const fn line_width(&self) -> FrameStrokeWidth {
        self.line_width
    }
}

/// Canonical post-composition opacity for a materialized Picture command.
///
/// Storing the normalized `f32` bits preserves the CPU rasterizer's exact
/// per-channel truncation while keeping the ordered command model `Eq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameOpacity(u32);

impl FrameOpacity {
    pub const fn opaque() -> Self {
        Self(1.0f32.to_bits())
    }

    pub fn from_canvas(opacity: f32) -> Self {
        let opacity = if opacity.is_nan() {
            0.0
        } else {
            opacity.clamp(0.0, 1.0)
        };
        Self(if opacity == 0.0 {
            0.0f32.to_bits()
        } else {
            opacity.to_bits()
        })
    }

    pub const fn value(self) -> f32 {
        f32::from_bits(self.0)
    }

    pub fn is_opaque(self) -> bool {
        self.value() >= 1.0 - 1e-6
    }

    pub const fn is_transparent(self) -> bool {
        self.0 == 0.0f32.to_bits()
    }
}

/// One validated integer-positioned glyph coverage blit.
///
/// The coverage allocation is retained by the command stream, so a recorded
/// frame stays valid even when the font cache evicts the glyph before submit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameGlyphBlit {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    color: Color,
    coverage: Arc<[u8]>,
}

impl FrameGlyphBlit {
    pub fn new(
        x: i32,
        y: i32,
        coverage: Arc<[u8]>,
        width: usize,
        height: usize,
        color: Color,
    ) -> Result<Self, FrameEncoderError> {
        let width_u32 =
            u32::try_from(width).map_err(|_| FrameEncoderError::InvalidGlyphCoverage {
                width,
                height,
                actual: coverage.len(),
            })?;
        let height_u32 =
            u32::try_from(height).map_err(|_| FrameEncoderError::InvalidGlyphCoverage {
                width,
                height,
                actual: coverage.len(),
            })?;
        let required = width.checked_mul(height).filter(|required| *required > 0);
        if required.is_none_or(|required| coverage.len() < required) {
            return Err(FrameEncoderError::InvalidGlyphCoverage {
                width,
                height,
                actual: coverage.len(),
            });
        }
        Ok(Self {
            x,
            y,
            width: width_u32,
            height: height_u32,
            color,
            coverage,
        })
    }

    pub const fn x(&self) -> i32 {
        self.x
    }

    pub const fn y(&self) -> i32 {
        self.y
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn color(&self) -> Color {
        self.color
    }

    pub fn coverage(&self) -> &Arc<[u8]> {
        &self.coverage
    }
}

/// A self-contained premultiplied-AARRGGBB CPU image used to model a
/// Picture/offscreen result. This matches the software rasterizer's pixel
/// representation, so reference execution can be copied into a CPU Picture
/// without a lossy color conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameImage {
    width: i32,
    height: i32,
    pixels: Arc<[u32]>,
}

impl FrameImage {
    pub fn new(width: i32, height: i32, pixels: Vec<u32>) -> Result<Self, FrameEncoderError> {
        let expected = pixel_len(width, height)?;
        if pixels.len() != expected {
            return Err(FrameEncoderError::PixelCountMismatch {
                width,
                height,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels: pixels.into(),
        })
    }

    pub fn solid(width: i32, height: i32, color: Color) -> Result<Self, FrameEncoderError> {
        Ok(Self {
            width,
            height,
            pixels: vec![color.premultiplied(); pixel_len(width, height)?].into(),
        })
    }

    pub const fn width(&self) -> i32 {
        self.width
    }

    pub const fn height(&self) -> i32 {
        self.height
    }

    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }
}

/// API-neutral raster work.  More operations can be added without changing
/// frame ordering or the presentation contract.
///
/// 目标相关操作（[`Self::FillRectAdditive`]、
/// [`Self::FillRoundedRectAdditive`]、[`Self::ScrollCopy`]）必须直接作用于
/// 累积目标（Native 命令或参考执行器）。把它们录进透明 CPU segment 后再做
/// source-over 合成并不等价，禁止作为替代实现。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameRasterOp {
    FillRect {
        rect: FrameRect,
        color: Color,
    },
    /// 使用共享 SDF coverage 对圆角区域执行普通 SrcOver 填充。
    FillRoundedRect {
        rect: FrameRect,
        color: Color,
        radius: FrameRadius,
    },
    /// 使用原始圆角几何，仅以整数 surface-space 矩形硬裁剪 SrcOver 覆盖。
    /// `radius == 0` 时同时承载普通矩形的 clipped fast path。
    FillRoundedRectClipped {
        rect: FrameRect,
        color: Color,
        radius: FrameRadius,
        clip: FrameRect,
    },
    /// Ordered SrcOver glyph coverage blits sharing one integer surface clip.
    BlitGlyphs {
        glyphs: Vec<FrameGlyphBlit>,
        clip: FrameRect,
    },
    /// Ordered centered SrcOver rectangle strokes sharing one surface clip.
    StrokeRoundedRects {
        strokes: Vec<FrameStrokeRect>,
        clip: FrameRect,
    },
    /// Channel-wise saturating add into the destination (CPU Additive blend).
    FillRectAdditive {
        rect: FrameRect,
        color: Color,
    },
    /// 使用共享 SDF coverage 对圆角区域执行逐通道饱和加法。
    FillRoundedRectAdditive {
        rect: FrameRect,
        color: Color,
        radius: FrameRadius,
    },
    /// 把 `viewport` 中按 `(dx, dy)` 平移后的像素复制回同一视口；
    /// 语义与 [`crate::draw::traits::Canvas2D::scroll_region`] 一致，位移取整。
    ScrollCopy {
        viewport: FrameRect,
        dx: i32,
        dy: i32,
    },
}

/// A single ordered frame command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameCommand {
    Clear {
        color: Color,
    },
    Native {
        operation: FrameRasterOp,
    },
    CpuSegment {
        image: FrameImage,
        src: FrameRect,
        dst: FrameRect,
    },
    PictureBlit {
        image: FrameImage,
        src: FrameRect,
        dst: FrameRect,
        opacity: FrameOpacity,
    },
}

/// CPU-produced raster payloads that are forbidden by a GPU-native frame.
///
/// Source images loaded by the application are not classified here: this
/// audit only tracks pixels generated by UI rasterization or Picture
/// materialization inside the draw pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuFrameViolationKind {
    CpuRasterSegment,
    CpuGlyphCoverage,
    MaterializedPicture,
}

impl std::fmt::Display for GpuFrameViolationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::CpuRasterSegment => "CPU raster segment",
            Self::CpuGlyphCoverage => "CPU-rasterized glyph coverage",
            Self::MaterializedPicture => "CPU-materialized Picture",
        })
    }
}

/// Allocation-free audit of one encoded frame's raster provenance.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuFrameAudit {
    pub cpu_raster_segments: usize,
    pub cpu_raster_bytes: usize,
    pub cpu_glyphs: usize,
    pub cpu_glyph_bytes: usize,
    pub materialized_pictures: usize,
    pub materialized_picture_bytes: usize,
}

impl GpuFrameAudit {
    pub const fn is_gpu_native(self) -> bool {
        self.cpu_raster_segments == 0 && self.cpu_glyphs == 0 && self.materialized_pictures == 0
    }

    pub const fn first_violation(self) -> Option<GpuFrameViolationKind> {
        if self.cpu_raster_segments != 0 {
            Some(GpuFrameViolationKind::CpuRasterSegment)
        } else if self.cpu_glyphs != 0 {
            Some(GpuFrameViolationKind::CpuGlyphCoverage)
        } else if self.materialized_pictures != 0 {
            Some(GpuFrameViolationKind::MaterializedPicture)
        } else {
            None
        }
    }
}

/// Result of one successful final presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentOutcome {
    Presented,
}

/// Result of asking a backend to execute an encoded Picture. Backends that do
/// not have a lossless executor explicitly return `Unsupported`; compositor
/// code then retains its normal DisplayList replay path rather than silently
/// changing the raster contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodedPictureExecution {
    Executed,
    Unsupported,
}

/// Result of asking a backend to execute the complete main-surface command
/// stream. Unlike [`EncodedPictureExecution`], this target is the frame that
/// will reach the sole final presenter; execution itself must not present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodedFrameExecution {
    Executed,
    Unsupported,
}

/// The sole final-submission boundary for an encoded frame.
pub trait FramePresenter {
    type Error;

    fn present(&mut self, frame: &ReferenceFrame) -> Result<PresentOutcome, Self::Error>;
}

/// Immutable, API-neutral frame produced by the in-memory reference executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceFrame {
    width: i32,
    height: i32,
    pixels: Vec<u32>,
}

impl ReferenceFrame {
    pub const fn width(&self) -> i32 {
        self.width
    }

    pub const fn height(&self) -> i32 {
        self.height
    }

    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    pub fn pixel(&self, x: i32, y: i32) -> Option<u32> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        self.pixels
            .get((y as usize).saturating_mul(self.width as usize) + x as usize)
            .copied()
    }
}

/// Recording errors that are deterministically detectable without a GPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameEncoderError {
    InvalidExtent {
        width: i32,
        height: i32,
    },
    PixelCountMismatch {
        width: i32,
        height: i32,
        actual: usize,
    },
    DestinationDependentCpuSegment {
        operation: &'static str,
    },
    InvalidRadius {
        corner: &'static str,
    },
    InvalidStrokeWidth,
    InvalidGlyphCoverage {
        width: usize,
        height: usize,
        actual: usize,
    },
    CommandAllocationFailed,
    GpuNativeViolation {
        kind: GpuFrameViolationKind,
        payload_bytes: usize,
    },
}

impl std::fmt::Display for FrameEncoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidExtent { width, height } => {
                write!(f, "frame extent must be positive, got {width}x{height}")
            }
            Self::PixelCountMismatch {
                width,
                height,
                actual,
            } => write!(
                f,
                "frame image {width}x{height} requires {} pixels, got {actual}",
                (*width as usize).saturating_mul(*height as usize)
            ),
            Self::DestinationDependentCpuSegment { operation } => write!(
                f,
                "{operation} depends on destination pixels and cannot be recorded as a transparent CPU segment"
            ),
            Self::InvalidRadius { corner } => write!(
                f,
                "frame radius {corner} must be finite and non-negative"
            ),
            Self::InvalidStrokeWidth => {
                write!(f, "frame stroke width must be finite and positive")
            }
            Self::InvalidGlyphCoverage {
                width,
                height,
                actual,
            } => write!(
                f,
                "glyph {width}x{height} requires a non-empty coverage payload of at least {} bytes, got {actual}",
                width.saturating_mul(*height)
            ),
            Self::CommandAllocationFailed => {
                write!(f, "frame command allocation exceeded available memory")
            }
            Self::GpuNativeViolation {
                kind,
                payload_bytes,
            } => write!(
                f,
                "GPU-native frame contains forbidden {kind} payload ({payload_bytes} bytes)"
            ),
        }
    }
}

impl std::error::Error for FrameEncoderError {}

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
        self.blit_picture_with_opacity(image, src, dst, FrameOpacity::opaque());
    }

    pub(crate) fn blit_picture_with_opacity(
        &mut self,
        image: FrameImage,
        src: FrameRect,
        dst: FrameRect,
        opacity: FrameOpacity,
    ) {
        self.commands.push(FrameCommand::PictureBlit {
            image,
            src,
            dst,
            opacity,
        });
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
                } => {
                    if opacity.is_transparent() {
                        continue;
                    }
                    if target_is_transparent
                        && opacity.is_opaque()
                        && full_frame_image_blit(self.width, self.height, image, *src, *dst)
                    {
                        // Source-over onto a transparent target is exactly the
                        // premultiplied source. Retained backdrop restores use
                        // this ordered shape, avoiding one alpha branch and
                        // blend decision per full-surface pixel.
                        pixels.copy_from_slice(image.pixels());
                    } else {
                        blit_image_pixels_with_opacity(
                            self.width,
                            self.height,
                            pixels,
                            image,
                            *src,
                            *dst,
                            opacity.value(),
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
        dst: FrameRect,
        opacity: FrameOpacity,
    ) -> Option<(ReferenceFrame, FrameRect)> {
        if opacity.is_transparent() {
            return None;
        }
        self.image_blit_reference_tile(image, src, dst, opacity.value())
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

#[derive(Clone, Copy)]
struct SourceOverWrite {
    rect: FrameRect,
}

fn stroke_visible_bounds(
    stroke: FrameStrokeRect,
    clip: FrameRect,
    width: i32,
    height: i32,
) -> Option<FrameRect> {
    if stroke.rect.is_empty() {
        return None;
    }
    let clip = clip.intersection(FrameRect::new(0, 0, width, height))?;
    let expand = stroke.line_width.value() * 0.5 + 1.0;
    let left = ((stroke.rect.x as f32 - expand).max(clip.x as f32)) as i32;
    let top = ((stroke.rect.y as f32 - expand).max(clip.y as f32)) as i32;
    let right = ((stroke.rect.x.saturating_add(stroke.rect.width) as f32 + expand)
        .min(clip.x.saturating_add(clip.width) as f32)) as i32;
    let bottom = ((stroke.rect.y.saturating_add(stroke.rect.height) as f32 + expand)
        .min(clip.y.saturating_add(clip.height) as f32)) as i32;
    (left < right && top < bottom).then(|| FrameRect::new(left, top, right - left, bottom - top))
}

fn stroke_batch_bounds(
    strokes: &[FrameStrokeRect],
    clip: FrameRect,
    width: i32,
    height: i32,
) -> Option<(FrameRect, i64)> {
    let mut union = None;
    let mut covered_area = 0i64;
    for stroke in strokes {
        let Some(bounds) = stroke_visible_bounds(*stroke, clip, width, height) else {
            continue;
        };
        covered_area = covered_area
            .saturating_add(i64::from(bounds.width).saturating_mul(i64::from(bounds.height)));
        union = Some(union.map_or(bounds, |previous| {
            union_nonempty_frame_rect(previous, bounds)
        }));
    }
    union.map(|bounds| (bounds, covered_area))
}

fn stroke_batches_can_merge(
    previous: &[FrameStrokeRect],
    next: &[FrameStrokeRect],
    clip: FrameRect,
    width: i32,
    height: i32,
) -> bool {
    const MAX_CLUSTER_ITEMS: usize = 2048;
    const MAX_CLUSTER_PIXELS: i64 = 1024 * 1024;
    const MAX_UNION_INFLATION: i64 = 4;
    if previous.len().saturating_add(next.len()) > MAX_CLUSTER_ITEMS {
        return false;
    }
    let Some((previous_bounds, previous_area)) = stroke_batch_bounds(previous, clip, width, height)
    else {
        return true;
    };
    let Some((next_bounds, next_area)) = stroke_batch_bounds(next, clip, width, height) else {
        return true;
    };
    for previous_stroke in previous {
        let Some(previous_visible) = stroke_visible_bounds(*previous_stroke, clip, width, height)
        else {
            continue;
        };
        for next_stroke in next {
            if stroke_visible_bounds(*next_stroke, clip, width, height)
                .and_then(|next_visible| previous_visible.intersection(next_visible))
                .is_some()
            {
                return false;
            }
        }
    }
    let union = union_nonempty_frame_rect(previous_bounds, next_bounds);
    let union_area = i64::from(union.width).saturating_mul(i64::from(union.height));
    let covered_area = previous_area.saturating_add(next_area);
    union_area <= MAX_CLUSTER_PIXELS
        && union_area <= covered_area.saturating_mul(MAX_UNION_INFLATION)
}

fn union_nonempty_frame_rect(a: FrameRect, b: FrameRect) -> FrameRect {
    let left = a.x.min(b.x);
    let top = a.y.min(b.y);
    let right = (i64::from(a.x) + i64::from(a.width)).max(i64::from(b.x) + i64::from(b.width));
    let bottom = (i64::from(a.y) + i64::from(a.height)).max(i64::from(b.y) + i64::from(b.height));
    FrameRect::new(
        left,
        top,
        (right - i64::from(left)) as i32,
        (bottom - i64::from(top)) as i32,
    )
}

fn source_over_commands_have_safe_grouping(
    commands: &[FrameCommand],
    width: i32,
    height: i32,
) -> bool {
    let mut writes = Vec::new();
    let mut opaque_covers = Vec::new();
    for command in commands {
        match command {
            FrameCommand::Clear { .. } => return false,
            FrameCommand::Native { operation } => match operation {
                FrameRasterOp::FillRect { rect, color } => {
                    let opaque = color.a == u8::MAX;
                    if !push_source_over_write(&mut writes, *rect, width, height) {
                        return false;
                    }
                    if opaque {
                        push_opaque_cover(&mut opaque_covers, *rect);
                    }
                }
                FrameRasterOp::FillRoundedRect {
                    rect,
                    color,
                    radius,
                } => {
                    if !push_source_over_write(&mut writes, *rect, width, height) {
                        return false;
                    }
                    if color.a == u8::MAX {
                        if let Some(inner) = rounded_rect_opaque_inner(*rect, *radius) {
                            push_opaque_cover(&mut opaque_covers, inner);
                        }
                    }
                }
                FrameRasterOp::FillRoundedRectClipped {
                    rect,
                    color,
                    radius,
                    clip,
                } => {
                    let Some(visible) = rect.intersection(*clip) else {
                        continue;
                    };
                    if !push_source_over_write(&mut writes, visible, width, height) {
                        return false;
                    }
                    if color.a == u8::MAX {
                        if let Some(inner) = rounded_rect_opaque_inner(*rect, *radius)
                            .and_then(|inner| inner.intersection(*clip))
                        {
                            push_opaque_cover(&mut opaque_covers, inner);
                        }
                    }
                }
                FrameRasterOp::BlitGlyphs { glyphs, clip } => {
                    if !clip.is_within(width, height) {
                        return false;
                    }
                    for glyph in glyphs {
                        let (Ok(glyph_width), Ok(glyph_height)) =
                            (i32::try_from(glyph.width), i32::try_from(glyph.height))
                        else {
                            return false;
                        };
                        let Some(visible) =
                            FrameRect::new(glyph.x, glyph.y, glyph_width, glyph_height)
                                .intersection(*clip)
                        else {
                            continue;
                        };
                        if !push_source_over_write(&mut writes, visible, width, height) {
                            return false;
                        }
                    }
                }
                FrameRasterOp::StrokeRoundedRects { .. } => return false,
                FrameRasterOp::FillRectAdditive { .. }
                | FrameRasterOp::FillRoundedRectAdditive { .. }
                | FrameRasterOp::ScrollCopy { .. } => return false,
            },
            FrameCommand::CpuSegment { image, src, dst }
            | FrameCommand::PictureBlit {
                image, src, dst, ..
            } => {
                if src.width != dst.width
                    || src.height != dst.height
                    || !src.is_within(image.width, image.height)
                    || !push_source_over_write(&mut writes, *dst, width, height)
                {
                    return false;
                }
            }
        }
    }

    for (index, write) in writes.iter().enumerate() {
        for previous in &writes[..index] {
            if let Some(overlap) = previous.rect.intersection(write.rect) {
                if !opaque_covers_rect(&opaque_covers, overlap) {
                    return false;
                }
            }
        }
    }
    true
}

fn push_opaque_cover(covers: &mut Vec<FrameRect>, rect: FrameRect) {
    if covers.try_reserve(1).is_ok() {
        covers.push(rect);
    }
}

fn opaque_covers_rect(covers: &[FrameRect], target: FrameRect) -> bool {
    if covers
        .iter()
        .any(|cover| cover.intersection(target) == Some(target))
    {
        return true;
    }

    let target_left = i64::from(target.x);
    let target_top = i64::from(target.y);
    let target_right = target_left + i64::from(target.width);
    let target_bottom = target_top + i64::from(target.height);
    let mut x = target_left;
    while x < target_right {
        let mut next_x = target_right;
        for cover in covers {
            let Some(clipped) = cover.intersection(target) else {
                continue;
            };
            let clipped_left = i64::from(clipped.x);
            let clipped_right = clipped_left + i64::from(clipped.width);
            for boundary in [clipped_left, clipped_right] {
                if boundary > x {
                    next_x = next_x.min(boundary);
                }
            }
        }

        let mut y = target_top;
        while y < target_bottom {
            let mut covered_until = y;
            for cover in covers {
                let cover_left = i64::from(cover.x);
                let cover_top = i64::from(cover.y);
                let cover_right = cover_left + i64::from(cover.width);
                let cover_bottom = cover_top + i64::from(cover.height);
                if cover_left <= x && cover_right >= next_x && cover_top <= y && cover_bottom > y {
                    covered_until = covered_until.max(cover_bottom.min(target_bottom));
                }
            }
            if covered_until == y {
                return false;
            }
            y = covered_until;
        }
        x = next_x;
    }
    true
}

fn rounded_rect_opaque_inner(rect: FrameRect, radius: FrameRadius) -> Option<FrameRect> {
    let radius = radius.to_radius();
    let inset = |value: f32| value.ceil() as i64;
    let left = inset(radius.tl.max(radius.bl));
    let right = inset(radius.tr.max(radius.br));
    let top = inset(radius.tl.max(radius.tr));
    let bottom = inset(radius.bl.max(radius.br));
    let inner_width = i64::from(rect.width)
        .checked_sub(left)?
        .checked_sub(right)?;
    let inner_height = i64::from(rect.height)
        .checked_sub(top)?
        .checked_sub(bottom)?;
    if inner_width <= 0 || inner_height <= 0 {
        return None;
    }
    Some(FrameRect::new(
        i32::try_from(i64::from(rect.x).checked_add(left)?).ok()?,
        i32::try_from(i64::from(rect.y).checked_add(top)?).ok()?,
        i32::try_from(inner_width).ok()?,
        i32::try_from(inner_height).ok()?,
    ))
}

fn push_source_over_write(
    writes: &mut Vec<SourceOverWrite>,
    rect: FrameRect,
    width: i32,
    height: i32,
) -> bool {
    if !rect.is_within(width, height) || writes.try_reserve(1).is_err() {
        return false;
    }
    writes.push(SourceOverWrite { rect });
    true
}

#[derive(Clone, Copy)]
struct PictureCropTranslation {
    source_width: i32,
    source_height: i32,
    source_crop: FrameRect,
    dx: i32,
    dy: i32,
    target_width: i32,
    target_height: i32,
}

fn crop_and_translate_source_over_command(
    command: &FrameCommand,
    translation: &PictureCropTranslation,
) -> Result<Option<FrameCommand>, ()> {
    let translate_original = |rect: FrameRect| {
        if !rect.is_within(translation.source_width, translation.source_height) {
            return Err(());
        }
        rect.translated(translation.dx, translation.dy).ok_or(())
    };
    let translate_visible = |rect: FrameRect| {
        if !rect.is_within(translation.source_width, translation.source_height) {
            return Err(());
        }
        let Some(visible) = rect.intersection(translation.source_crop) else {
            return Ok(None);
        };
        let translated = visible
            .translated(translation.dx, translation.dy)
            .ok_or(())?;
        if !translated.is_within(translation.target_width, translation.target_height) {
            return Err(());
        }
        Ok(Some((visible, translated)))
    };

    Ok(Some(match command {
        FrameCommand::Clear { .. } => return Err(()),
        FrameCommand::Native { operation } => {
            let operation = match operation {
                FrameRasterOp::FillRect { rect, color } => {
                    let Some((_, rect)) = translate_visible(*rect)? else {
                        return Ok(None);
                    };
                    FrameRasterOp::FillRect {
                        rect,
                        color: *color,
                    }
                }
                FrameRasterOp::FillRoundedRect {
                    rect,
                    color,
                    radius,
                } => {
                    let Some((visible, translated_visible)) = translate_visible(*rect)? else {
                        return Ok(None);
                    };
                    let translated_rect = translate_original(*rect)?;
                    if visible == *rect {
                        FrameRasterOp::FillRoundedRect {
                            rect: translated_rect,
                            color: *color,
                            radius: *radius,
                        }
                    } else {
                        FrameRasterOp::FillRoundedRectClipped {
                            rect: translated_rect,
                            color: *color,
                            radius: *radius,
                            clip: translated_visible,
                        }
                    }
                }
                FrameRasterOp::FillRoundedRectClipped {
                    rect,
                    color,
                    radius,
                    clip,
                } => {
                    let Some((_, translated_clip)) = translate_visible(*clip)? else {
                        return Ok(None);
                    };
                    if rect
                        .intersection(*clip)
                        .and_then(|visible| visible.intersection(translation.source_crop))
                        .is_none()
                    {
                        return Ok(None);
                    }
                    FrameRasterOp::FillRoundedRectClipped {
                        rect: translate_original(*rect)?,
                        color: *color,
                        radius: *radius,
                        clip: translated_clip,
                    }
                }
                FrameRasterOp::BlitGlyphs { glyphs, clip } => {
                    let Some((visible_clip, translated_clip)) = translate_visible(*clip)? else {
                        return Ok(None);
                    };
                    let mut translated_glyphs = Vec::new();
                    translated_glyphs
                        .try_reserve_exact(glyphs.len())
                        .map_err(|_| ())?;
                    for glyph in glyphs {
                        let width = i32::try_from(glyph.width).map_err(|_| ())?;
                        let height = i32::try_from(glyph.height).map_err(|_| ())?;
                        if FrameRect::new(glyph.x, glyph.y, width, height)
                            .intersection(visible_clip)
                            .is_none()
                        {
                            continue;
                        }
                        translated_glyphs.push(FrameGlyphBlit {
                            x: glyph.x.checked_add(translation.dx).ok_or(())?,
                            y: glyph.y.checked_add(translation.dy).ok_or(())?,
                            width: glyph.width,
                            height: glyph.height,
                            color: glyph.color,
                            coverage: Arc::clone(&glyph.coverage),
                        });
                    }
                    if translated_glyphs.is_empty() {
                        return Ok(None);
                    }
                    FrameRasterOp::BlitGlyphs {
                        glyphs: translated_glyphs,
                        clip: translated_clip,
                    }
                }
                FrameRasterOp::StrokeRoundedRects { .. } => return Err(()),
                FrameRasterOp::FillRectAdditive { .. }
                | FrameRasterOp::FillRoundedRectAdditive { .. }
                | FrameRasterOp::ScrollCopy { .. } => return Err(()),
            };
            FrameCommand::Native { operation }
        }
        FrameCommand::CpuSegment { image, src, dst }
        | FrameCommand::PictureBlit {
            image, src, dst, ..
        } => {
            if src.width != dst.width
                || src.height != dst.height
                || !src.is_within(image.width, image.height)
            {
                return Err(());
            }
            let Some((visible_dst, translated_dst)) = translate_visible(*dst)? else {
                return Ok(None);
            };
            let source_x = src
                .x
                .checked_add(visible_dst.x.checked_sub(dst.x).ok_or(())?)
                .ok_or(())?;
            let source_y = src
                .y
                .checked_add(visible_dst.y.checked_sub(dst.y).ok_or(())?)
                .ok_or(())?;
            let translated_src =
                FrameRect::new(source_x, source_y, visible_dst.width, visible_dst.height);
            if !translated_src.is_within(image.width, image.height) {
                return Err(());
            }
            match command {
                FrameCommand::CpuSegment { .. } => FrameCommand::CpuSegment {
                    image: image.clone(),
                    src: translated_src,
                    dst: translated_dst,
                },
                FrameCommand::PictureBlit { opacity, .. } => FrameCommand::PictureBlit {
                    image: image.clone(),
                    src: translated_src,
                    dst: translated_dst,
                    opacity: *opacity,
                },
                _ => unreachable!(),
            }
        }
    }))
}

fn full_frame_image_blit(
    width: i32,
    height: i32,
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
) -> bool {
    image.width == width
        && image.height == height
        && src == FrameRect::new(0, 0, width, height)
        && dst == FrameRect::new(0, 0, width, height)
}

fn pixel_len(width: i32, height: i32) -> Result<usize, FrameEncoderError> {
    if width <= 0 || height <= 0 {
        return Err(FrameEncoderError::InvalidExtent { width, height });
    }
    let pixels = i64::from(width) * i64::from(height);
    usize::try_from(pixels).map_err(|_| FrameEncoderError::InvalidExtent { width, height })
}

fn apply_stroke_rect_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    stroke: FrameStrokeRect,
    clip: FrameRect,
) {
    if stroke.rect.is_empty() {
        return;
    }
    crate::draw::rasterizer::stroke::stroke_rect(
        pixels,
        width,
        height,
        Rect::new(
            clip.x as f32,
            clip.y as f32,
            clip.width as f32,
            clip.height as f32,
        ),
        1.0,
        Rect::new(
            stroke.rect.x as f32,
            stroke.rect.y as f32,
            stroke.rect.width as f32,
            stroke.rect.height as f32,
        ),
        stroke.color,
        stroke.line_width.value(),
        Some(stroke.radius.to_radius()),
    );
}

fn apply_raster_op_pixels(width: i32, height: i32, pixels: &mut [u32], operation: &FrameRasterOp) {
    match operation {
        FrameRasterOp::FillRect { rect, color } => {
            fill_rect_pixels(width, height, pixels, *rect, *color)
        }
        FrameRasterOp::FillRoundedRect {
            rect,
            color,
            radius,
        } => fill_rounded_rect_pixels(width, height, pixels, *rect, *color, *radius),
        FrameRasterOp::FillRoundedRectClipped {
            rect,
            color,
            radius,
            clip,
        } => {
            if let Some(clip) = clip.intersection(FrameRect::new(0, 0, width, height)) {
                fill_rounded_rect_pixels_clipped(
                    width, height, pixels, *rect, *color, *radius, clip,
                )
            }
        }
        FrameRasterOp::BlitGlyphs { glyphs, clip } => {
            let clip = Rect::new(
                clip.x as f32,
                clip.y as f32,
                clip.width as f32,
                clip.height as f32,
            );
            for glyph in glyphs {
                crate::draw::rasterizer::glyph::blit_glyph(
                    pixels,
                    width,
                    height,
                    clip,
                    1.0,
                    glyph.x,
                    glyph.y,
                    glyph.coverage.as_ref(),
                    glyph.width as usize,
                    glyph.height as usize,
                    glyph.color,
                );
            }
        }
        FrameRasterOp::StrokeRoundedRects { strokes, clip } => {
            for stroke in strokes {
                apply_stroke_rect_pixels(width, height, pixels, *stroke, *clip);
            }
        }
        FrameRasterOp::FillRectAdditive { rect, color } => {
            fill_rect_additive_pixels(width, height, pixels, *rect, *color)
        }
        FrameRasterOp::FillRoundedRectAdditive {
            rect,
            color,
            radius,
        } => fill_rounded_rect_additive_pixels(width, height, pixels, *rect, *color, *radius),
        FrameRasterOp::ScrollCopy { viewport, dx, dy } => {
            scroll_copy_pixels(width, height, pixels, *viewport, *dx, *dy)
        }
    }
}

/// Applies one destination-dependent (or ordinary) raster op onto an existing
/// premultiplied pixel buffer. Used by CPU execution and by NativeGpu when it
/// lowers Additive/Scroll through readback → reference op → upload.
pub(crate) fn apply_frame_raster_op(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    operation: &FrameRasterOp,
) {
    apply_raster_op_pixels(width, height, pixels, operation);
}

fn fill_rect_pixels(width: i32, height: i32, pixels: &mut [u32], rect: FrameRect, color: Color) {
    if rect.is_empty() {
        return;
    }
    let x0 = rect.x.max(0);
    let y0 = rect.y.max(0);
    let x1 = rect.x.saturating_add(rect.width).min(width);
    let y1 = rect.y.saturating_add(rect.height).min(height);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let source = color.premultiplied();
    let source_a = source >> 24;
    if source_a == 0 {
        return;
    }
    if source_a == 0xff {
        let row_width = width as usize;
        for y in y0..y1 {
            let start = y as usize * row_width + x0 as usize;
            pixels[start..start + (x1 - x0) as usize].fill(source);
        }
        return;
    }
    for y in y0..y1 {
        for x in x0..x1 {
            let index = y as usize * width as usize + x as usize;
            pixels[index] = blend_pixel_src_over(source, pixels[index]);
        }
    }
}

fn fill_rect_additive_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    rect: FrameRect,
    color: Color,
) {
    if rect.is_empty() {
        return;
    }
    let x0 = rect.x.max(0);
    let y0 = rect.y.max(0);
    let x1 = rect.x.saturating_add(rect.width).min(width);
    let y1 = rect.y.saturating_add(rect.height).min(height);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let source = color.premultiplied();
    for y in y0..y1 {
        for x in x0..x1 {
            let index = y as usize * width as usize + x as usize;
            pixels[index] = blend_pixel_additive(source, pixels[index]);
        }
    }
}

fn fill_rounded_rect_additive_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    rect: FrameRect,
    color: Color,
    radius: FrameRadius,
) {
    if rect.is_empty() {
        return;
    }
    let mut renderer = RasterRenderer::new(width, height);
    renderer.set_blend_mode(BlendMode::Additive);
    renderer.fill_rect(
        pixels,
        width,
        height,
        Rect::new(
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
        ),
        color,
        Some(radius.to_radius()),
    );
}

fn fill_rounded_rect_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    rect: FrameRect,
    color: Color,
    radius: FrameRadius,
) {
    if rect.is_empty() {
        return;
    }
    let renderer = RasterRenderer::new(width, height);
    renderer.fill_rect(
        pixels,
        width,
        height,
        Rect::new(
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
        ),
        color,
        Some(radius.to_radius()),
    );
}

fn fill_rounded_rect_pixels_clipped(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    rect: FrameRect,
    color: Color,
    radius: FrameRadius,
    clip: FrameRect,
) {
    if rect.is_empty() || clip.is_empty() {
        return;
    }
    let mut renderer = RasterRenderer::new(width, height);
    renderer.push_clip_surface(Rect::new(
        clip.x as f32,
        clip.y as f32,
        clip.width as f32,
        clip.height as f32,
    ));
    renderer.fill_rect(
        pixels,
        width,
        height,
        Rect::new(
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
        ),
        color,
        Some(radius.to_radius()),
    );
}

fn scroll_copy_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    viewport: FrameRect,
    dx: i32,
    dy: i32,
) {
    if viewport.is_empty() || (dx == 0 && dy == 0) {
        return;
    }
    // Match SharedRasterizer::scroll_region: source is viewport shifted by
    // (dx, dy), destination is the viewport origin.
    let src = FrameRect::new(
        viewport.x.saturating_add(dx),
        viewport.y.saturating_add(dy),
        viewport.width,
        viewport.height,
    );
    let src_x = src.x;
    let src_y = src.y;
    let copy_w = src.width;
    let copy_h = src.height;
    let dst_x = viewport.x;
    let dst_y = viewport.y;

    let clip_x0 = src_x.max(0).max(src_x - dst_x);
    let clip_y0 = src_y.max(0).max(src_y - dst_y);
    let clip_x1 = (src_x + copy_w).min(width).min(width + src_x - dst_x);
    let clip_y1 = (src_y + copy_h).min(height).min(height + src_y - dst_y);
    if clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
        return;
    }
    let row_len = (clip_x1 - clip_x0) as usize;
    if dst_y <= src_y {
        for row in clip_y0..clip_y1 {
            let src_idx = (row * width + clip_x0) as usize;
            let dst_idx = ((row + dst_y - src_y) * width + (clip_x0 + dst_x - src_x)) as usize;
            pixels.copy_within(src_idx..src_idx + row_len, dst_idx);
        }
    } else {
        for row in (clip_y0..clip_y1).rev() {
            let src_idx = (row * width + clip_x0) as usize;
            let dst_idx = ((row + dst_y - src_y) * width + (clip_x0 + dst_x - src_x)) as usize;
            pixels.copy_within(src_idx..src_idx + row_len, dst_idx);
        }
    }
}

fn blit_image_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
) {
    blit_image_pixels_impl::<false>(width, height, pixels, image, src, dst, 1.0);
}

fn blit_image_pixels_with_opacity(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
    opacity: f32,
) {
    if opacity >= 1.0 - 1e-6 {
        blit_image_pixels_impl::<false>(width, height, pixels, image, src, dst, 1.0);
    } else {
        blit_image_pixels_impl::<true>(width, height, pixels, image, src, dst, opacity);
    }
}

fn blit_image_pixels_impl<const APPLY_OPACITY: bool>(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
    opacity: f32,
) {
    if src.is_empty() || dst.is_empty() {
        return;
    }
    if src.width == dst.width && src.height == dst.height {
        blit_unscaled_image_pixels::<APPLY_OPACITY>(
            width, height, pixels, image, src, dst, opacity,
        );
        return;
    }
    let x0 = dst.x.max(0);
    let y0 = dst.y.max(0);
    let x1 = dst.x.saturating_add(dst.width).min(width);
    let y1 = dst.y.saturating_add(dst.height).min(height);
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    for y in y0..y1 {
        for x in x0..x1 {
            let local_x = x - dst.x;
            let local_y = y - dst.y;
            let source_x = src.x + local_x.saturating_mul(src.width) / dst.width;
            let source_y = src.y + local_y.saturating_mul(src.height) / dst.height;
            if source_x < 0 || source_y < 0 || source_x >= image.width || source_y >= image.height {
                continue;
            }
            let source = image.pixels[source_y as usize * image.width as usize + source_x as usize];
            let source = if APPLY_OPACITY {
                crate::draw::rasterizer::apply_opacity(source, opacity)
            } else {
                source
            };
            let index = y as usize * width as usize + x as usize;
            pixels[index] = blend_pixel_src_over(source, pixels[index]);
        }
    }
}

fn blit_unscaled_image_pixels<const APPLY_OPACITY: bool>(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
    opacity: f32,
) {
    // CPU segment 与大多数 Picture blit 都是同尺寸搬运；先同时裁目标与源，
    // 再按连续行处理，避免热路径逐像素整数除法与边界判断。
    let x0 = dst.x.max(0).max(dst.x.saturating_sub(src.x));
    let y0 = dst.y.max(0).max(dst.y.saturating_sub(src.y));
    let x1 = dst
        .x
        .saturating_add(dst.width)
        .min(width)
        .min(dst.x.saturating_add(image.width).saturating_sub(src.x));
    let y1 = dst
        .y
        .saturating_add(dst.height)
        .min(height)
        .min(dst.y.saturating_add(image.height).saturating_sub(src.y));
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let copy_width = (x1 - x0) as usize;
    for y in y0..y1 {
        let source_x = src.x + x0 - dst.x;
        let source_y = src.y + y - dst.y;
        let source_start = source_y as usize * image.width as usize + source_x as usize;
        let destination_start = y as usize * width as usize + x0 as usize;
        let source_row = &image.pixels[source_start..source_start + copy_width];
        let destination_row = &mut pixels[destination_start..destination_start + copy_width];
        for (&source, destination) in source_row.iter().zip(destination_row) {
            let source = if APPLY_OPACITY {
                crate::draw::rasterizer::apply_opacity(source, opacity)
            } else {
                source
            };
            match source >> 24 {
                0 => {}
                0xff => *destination = source,
                _ => *destination = blend_pixel_src_over(source, *destination),
            }
        }
    }
}

fn blend_pixel_src_over(source: u32, destination: u32) -> u32 {
    let source_a = (source >> 24) & 0xff;
    if source_a == 0 {
        return destination;
    }
    if source_a == 0xff {
        return source;
    }
    let destination_a = (destination >> 24) & 0xff;
    crate::draw::rasterizer::core::blend_srcover(
        source_a,
        destination_a,
        (source >> 16) & 0xff,
        (source >> 8) & 0xff,
        source & 0xff,
        (destination >> 16) & 0xff,
        (destination >> 8) & 0xff,
        destination & 0xff,
    )
}

fn blend_pixel_additive(source: u32, destination: u32) -> u32 {
    let source_a = (source >> 24) & 0xff;
    if source_a == 0 {
        return destination;
    }
    let add = |shift: u32| (((source >> shift) & 0xff) + ((destination >> shift) & 0xff)).min(0xff);
    (add(24) << 24) | (add(16) << 16) | (add(8) << 8) | add(0)
}
