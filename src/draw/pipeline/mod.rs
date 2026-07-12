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
    FrameImage, FramePresenter, FrameRasterOp, FrameRect, PresentOutcome, ReferenceFrame,
};
pub use invalidation::{
    Invalidation, InvalidationQueue, InvalidationQueueHandle, NodeId, ScrollDelta,
    invalidate_paint_handle,
};
pub use metrics::{InvalidationSource, RenderMetrics};
pub use render_frame::{FrameRenderInput, FrameRenderOutput, FrameRenderer};
pub use session::RenderSession;
