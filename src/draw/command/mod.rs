//! 后端无关的绘制命令、帧编码与录制。

pub mod display_list;
pub mod encoder;
pub(crate) mod recorder;

pub use display_list::{DisplayList, PaintOp, PaintPass};
pub use encoder::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FrameEncoder, FrameEncoderError,
    FrameGlyphBlit, FrameImage, FrameOpacity, FramePresenter, FrameRadius, FrameRasterOp,
    FrameRect, FrameSampledRect, FrameStrokeRect, FrameStrokeWidth, GpuFrameAudit,
    GpuFrameViolationKind, PresentOutcome, ReferenceFrame,
};
