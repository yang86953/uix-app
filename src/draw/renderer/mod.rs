//! 单一 Renderer、帧生命周期与调度状态。

pub mod animation;
pub mod bootstrap;
pub(crate) mod factory;
pub mod invalidation;
pub mod lifecycle;
pub mod metrics;
mod outcome;
pub mod recovery;
pub mod recovery_driver;
mod runtime;
pub mod scene_pipeline;
pub mod session;
pub mod target;
#[cfg(feature = "test-harness")]
pub(crate) mod test_harness;

pub use animation::AnimationRegistry;
pub use invalidation::{
    invalidate_paint_handle, Invalidation, InvalidationQueue, InvalidationQueueHandle, NodeId,
    ScrollDelta,
};
pub use lifecycle::{begin_frame, end_frame, normalize_strategy};
pub use metrics::{InvalidationSource, RenderMetrics};
pub use outcome::{GraphicsFailure, RenderOutcome};
pub use recovery::{GraphicsRecovery, RecoveryAction};
pub(crate) use recovery_driver::RebuildRequest;
pub use recovery_driver::{RecoveryDriver, RenderTargetRebuilder};
pub use runtime::Renderer;
pub use scene_pipeline::{FrameRenderInput, FrameRenderOutput, ScenePipeline};
pub use session::RenderSession;
pub use target::{
    GraphicsCapabilities, PresentationMode, RasterPipeline, RenderTarget, ScrollCopy,
    UpdateStrategy,
};
