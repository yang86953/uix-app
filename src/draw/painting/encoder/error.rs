//! 帧编码错误契约 — encoder 子模块。

use super::commands::GpuFrameViolationKind;

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
    InvalidSampledRect,
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
            Self::InvalidRadius { corner } => {
                write!(f, "frame radius {corner} must be finite and non-negative")
            }
            Self::InvalidStrokeWidth => {
                write!(f, "frame stroke width must be finite and positive")
            }
            Self::InvalidSampledRect => {
                write!(
                    f,
                    "sampled picture destination must be finite with positive width/height"
                )
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
