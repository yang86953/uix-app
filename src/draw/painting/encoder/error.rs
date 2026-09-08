//! 帧编码错误契约 — encoder 子模块。

use super::commands::GpuFrameViolationKind;

/// Recording errors that are deterministically detectable without a GPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameEncoderError {
    /// 图像或目标宽高不是正整数。
    InvalidExtent {
        /// 收到的像素宽度。
        width: i32,
        /// 收到的像素高度。
        height: i32,
    },
    /// 像素载荷数量与声明的二维尺寸不一致。
    PixelCountMismatch {
        /// 声明的像素宽度。
        width: i32,
        /// 声明的像素高度。
        height: i32,
        /// 实际提供的像素数量。
        actual: usize,
    },
    /// 依赖目标像素的操作无法记录为透明 CPU 片段。
    DestinationDependentCpuSegment {
        /// 触发目标像素依赖的操作名称。
        operation: &'static str,
    },
    /// 圆角半径不是有限非负值。
    InvalidRadius {
        /// 包含非法半径的圆角名称。
        corner: &'static str,
    },
    /// 描边宽度不是有限正值。
    InvalidStrokeWidth,
    /// Picture 采样目标矩形包含非有限坐标或非正尺寸。
    InvalidSampledRect,
    /// 字形覆盖率载荷不足以覆盖声明的像素尺寸。
    InvalidGlyphCoverage {
        /// 声明的字形像素宽度。
        width: usize,
        /// 声明的字形像素高度。
        height: usize,
        /// 实际提供的覆盖率字节数。
        actual: usize,
    },
    /// 记录帧命令时内存分配失败。
    CommandAllocationFailed,
    /// GPU 原生帧包含禁止的 CPU 物化载荷。
    GpuNativeViolation {
        /// 检测到的禁止载荷分类。
        kind: GpuFrameViolationKind,
        /// 禁止载荷占用的字节数。
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

impl std::error::Error for FrameEncoderError {}
