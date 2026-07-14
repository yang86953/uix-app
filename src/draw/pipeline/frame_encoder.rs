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

/// A self-contained premultiplied-AARRGGBB CPU image used to model a
/// Picture/offscreen result. This matches the software rasterizer's pixel
/// representation, so reference execution can be copied into a CPU Picture
/// without a lossy color conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameImage {
    width: i32,
    height: i32,
    pixels: Vec<u32>,
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
            pixels,
        })
    }

    pub fn solid(width: i32, height: i32, color: Color) -> Result<Self, FrameEncoderError> {
        Ok(Self {
            width,
            height,
            pixels: vec![color.premultiplied(); pixel_len(width, height)?],
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
    /// Copy pixels from `viewport` translated by `(dx, dy)` into `viewport`
    /// (same semantics as [`Canvas2D::scroll_region`] with rounded deltas).
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
    },
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

    pub fn clear(&mut self, color: Color) {
        self.commands.push(FrameCommand::Clear { color });
    }

    pub fn native(&mut self, operation: FrameRasterOp) {
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
            FrameRasterOp::FillRect { .. } => None,
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
                pixels: image.pixels,
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
        self.commands
            .push(FrameCommand::PictureBlit { image, src, dst });
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
        for command in &self.commands {
            match command {
                // Clear is a replace operation, never transparent source-over.
                FrameCommand::Clear { color } => pixels.fill(color.premultiplied()),
                FrameCommand::Native { operation } => {
                    apply_raster_op_pixels(self.width, self.height, pixels, operation)
                }
                FrameCommand::CpuSegment { image, src, dst } => {
                    blit_image_pixels(self.width, self.height, pixels, image, *src, *dst)
                }
                FrameCommand::PictureBlit { image, src, dst } => {
                    blit_image_pixels(self.width, self.height, pixels, image, *src, *dst)
                }
            }
        }
    }

    /// Rasterizes one image segment into a transparent frame-sized source.
    /// Native executors alpha-blit this exact segment at its recorded point in
    /// the command order instead of uploading the completed frame.
    pub(crate) fn cpu_segment_reference(
        &self,
        image: &FrameImage,
        src: FrameRect,
        dst: FrameRect,
    ) -> ReferenceFrame {
        let mut frame = self.transparent_reference();
        blit_image_pixels(self.width, self.height, &mut frame.pixels, image, src, dst);
        frame
    }

    /// Rasterizes one Picture blit into a transparent frame-sized source for
    /// API-native execution at its exact painter-order boundary.
    pub(crate) fn picture_blit_reference(
        &self,
        image: &FrameImage,
        src: FrameRect,
        dst: FrameRect,
    ) -> ReferenceFrame {
        let mut frame = self.transparent_reference();
        blit_image_pixels(self.width, self.height, &mut frame.pixels, image, src, dst);
        frame
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

fn pixel_len(width: i32, height: i32) -> Result<usize, FrameEncoderError> {
    if width <= 0 || height <= 0 {
        return Err(FrameEncoderError::InvalidExtent { width, height });
    }
    let pixels = i64::from(width) * i64::from(height);
    usize::try_from(pixels).map_err(|_| FrameEncoderError::InvalidExtent { width, height })
}

fn apply_raster_op_pixels(width: i32, height: i32, pixels: &mut [u32], operation: &FrameRasterOp) {
    match operation {
        FrameRasterOp::FillRect { rect, color } => {
            fill_rect_pixels(width, height, pixels, *rect, *color)
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
    for y in y0..y1 {
        for x in x0..x1 {
            let index = y as usize * width as usize + x as usize;
            pixels[index] = blend_pixel_src_over(color.premultiplied(), pixels[index]);
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
    if src.is_empty() || dst.is_empty() {
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
            let index = y as usize * width as usize + x as usize;
            pixels[index] = blend_pixel_src_over(source, pixels[index]);
        }
    }
}

fn blend_pixel_src_over(source: u32, destination: u32) -> u32 {
    let source_a = (source >> 24) & 0xff;
    if source_a == 0 {
        return destination;
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
