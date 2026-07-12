//! API-neutral ordered frame command model.
//!
//! This is deliberately independent from any graphics context.  It provides
//! the R1 contract foundation: one ordered stream for clear, native work, CPU
//! fallback segments, and Picture/offscreen blits; the only presentation entry
//! consumes the encoder, so a recorded frame cannot be presented twice.

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameRasterOp {
    FillRect { rect: FrameRect, color: Color },
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
        operations: Vec<FrameRasterOp>,
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

    /// Records one bounded CPU fallback segment at its exact painter-order
    /// position.  Empty segments are ignored so they cannot create a second
    /// presentation boundary.
    pub fn cpu_segment(&mut self, operations: impl IntoIterator<Item = FrameRasterOp>) {
        let operations = operations.into_iter().collect::<Vec<_>>();
        if !operations.is_empty() {
            self.commands.push(FrameCommand::CpuSegment { operations });
        }
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
    pub(crate) fn execute_into_pixels(&self, pixels: &mut [u32]) {
        assert_eq!(
            pixels.len(),
            self.pixel_count,
            "FrameEncoder target must match its recorded extent"
        );
        pixels.fill(Color::transparent().premultiplied());
        for command in &self.commands {
            match command {
                // Clear is a replace operation, never transparent source-over.
                FrameCommand::Clear { color } => pixels.fill(color.premultiplied()),
                FrameCommand::Native { operation } => {
                    apply_raster_op_pixels(self.width, self.height, pixels, operation)
                }
                FrameCommand::CpuSegment { operations } => {
                    for operation in operations {
                        apply_raster_op_pixels(self.width, self.height, pixels, operation);
                    }
                }
                FrameCommand::PictureBlit { image, src, dst } => {
                    blit_image_pixels(self.width, self.height, pixels, image, *src, *dst)
                }
            }
        }
    }

    /// Rasterizes one CPU fallback segment into a transparent frame-sized
    /// source. Native executors alpha-blit this exact segment at its recorded
    /// point in the command order instead of uploading the completed frame.
    pub(crate) fn cpu_segment_reference(&self, operations: &[FrameRasterOp]) -> ReferenceFrame {
        let mut frame = self.transparent_reference();
        for operation in operations {
            apply_raster_op_pixels(self.width, self.height, &mut frame.pixels, operation);
        }
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
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: i32, height: i32, color: Color) -> FrameRasterOp {
        FrameRasterOp::FillRect {
            rect: FrameRect::new(x, y, width, height),
            color,
        }
    }

    #[test]
    fn clear_picture_native_draw_follows_painter_order() {
        let mut encoder = FrameEncoder::new(4, 4).unwrap();
        encoder.clear(Color::black());
        encoder.blit_picture(
            FrameImage::solid(4, 4, Color::blue()).unwrap(),
            FrameRect::new(0, 0, 4, 4),
            FrameRect::new(0, 0, 4, 4),
        );
        encoder.native(rect(1, 1, 2, 2, Color::red()));

        let frame = encoder.render_reference();
        assert_eq!(frame.pixel(0, 0), Some(Color::blue().to_rgba()));
        assert_eq!(frame.pixel(1, 1), Some(Color::red().to_rgba()));
        assert_eq!(frame.pixel(2, 2), Some(Color::red().to_rgba()));
    }

    #[test]
    fn native_draw_picture_blit_follows_painter_order() {
        let mut encoder = FrameEncoder::new(4, 4).unwrap();
        encoder.clear(Color::black());
        encoder.native(rect(1, 1, 2, 2, Color::red()));
        encoder.blit_picture(
            FrameImage::solid(4, 4, Color::blue()).unwrap(),
            FrameRect::new(0, 0, 4, 4),
            FrameRect::new(0, 0, 4, 4),
        );

        let frame = encoder.render_reference();
        assert_eq!(frame.pixel(1, 1), Some(Color::blue().to_rgba()));
        assert_eq!(frame.pixel(3, 3), Some(Color::blue().to_rgba()));
    }

    #[test]
    fn native_cpu_segment_native_preserves_the_full_command_order() {
        let mut encoder = FrameEncoder::new(5, 1).unwrap();
        encoder.clear(Color::black());
        encoder.native(rect(0, 0, 5, 1, Color::red()));
        encoder.cpu_segment([rect(1, 0, 3, 1, Color::green())]);
        encoder.native(rect(2, 0, 1, 1, Color::blue()));

        let frame = encoder.render_reference();
        assert_eq!(
            frame.pixels(),
            &[
                Color::red().to_rgba(),
                Color::green().to_rgba(),
                Color::blue().to_rgba(),
                Color::green().to_rgba(),
                Color::red().to_rgba(),
            ]
        );
    }

    struct RecordingPresenter {
        calls: usize,
        frame: Option<ReferenceFrame>,
    }

    impl FramePresenter for RecordingPresenter {
        type Error = ();

        fn present(&mut self, frame: &ReferenceFrame) -> Result<PresentOutcome, Self::Error> {
            self.calls += 1;
            self.frame = Some(frame.clone());
            Ok(PresentOutcome::Presented)
        }
    }

    #[test]
    fn consuming_encoder_submits_exactly_one_frame() {
        let mut encoder = FrameEncoder::new(2, 1).unwrap();
        encoder.clear(Color::black());
        encoder.native(rect(0, 0, 1, 1, Color::white()));
        let mut presenter = RecordingPresenter {
            calls: 0,
            frame: None,
        };

        let outcome = encoder.present(&mut presenter).unwrap();

        assert_eq!(outcome, PresentOutcome::Presented);
        assert_eq!(presenter.calls, 1);
        assert_eq!(
            presenter.frame.unwrap().pixels(),
            &[Color::white().to_rgba(), Color::black().to_rgba()]
        );
    }

    #[test]
    fn reference_executor_matches_cpu_rasterizer_for_transparent_rect_layers() {
        use crate::core::Rect;
        use crate::draw::engine::cpu::pixel_surface::PixelSurface;
        use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
        use crate::draw::traits::Canvas2D;

        let clear = Color::from_rgba(16, 32, 64, 96);
        let first = Color::from_rgba(255, 0, 0, 128);
        let second = Color::from_rgba(0, 96, 255, 144);
        let mut encoder = FrameEncoder::new(4, 3).unwrap();
        encoder.clear(clear);
        encoder.native(rect(-1, 0, 3, 3, first));
        encoder.cpu_segment([rect(1, 1, 4, 3, second)]);

        let mut cpu = SharedRasterizer::new(PixelSurface::new(4, 3));
        cpu.surface_mut().set_clear_color(clear);
        cpu.surface_mut().clear_all();
        cpu.fill_rect(Rect::new(-1.0, 0.0, 3.0, 3.0), first, None);
        cpu.fill_rect(Rect::new(1.0, 1.0, 4.0, 3.0), second, None);

        assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
    }
}
