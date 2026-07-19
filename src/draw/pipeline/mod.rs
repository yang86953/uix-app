//! 帧调度管线。

pub mod animation_registry;
pub mod frame;
pub mod frame_encoder;
pub(crate) mod frame_recording;
pub mod invalidation;
pub mod metrics;
pub mod render_frame;
pub mod session;

pub use animation_registry::AnimationRegistry;
pub use frame::{begin_frame, end_frame, normalize_strategy};
pub use frame_encoder::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FrameEncoder, FrameEncoderError,
    FrameGlyphBlit, FrameImage, FramePresenter, FrameRadius, FrameRasterOp, FrameRect,
    PresentOutcome, ReferenceFrame,
};
pub use invalidation::{
    invalidate_paint_handle, Invalidation, InvalidationQueue, InvalidationQueueHandle, NodeId,
    ScrollDelta,
};
pub use metrics::{InvalidationSource, RenderMetrics};
pub use render_frame::{FrameRenderInput, FrameRenderOutput, FrameRenderer};
pub use session::RenderSession;
