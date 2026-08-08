//! 单一 Renderer、帧生命周期与调度状态。

pub mod animation;
pub(crate) mod bootstrap;
pub(crate) mod factory;
pub mod invalidation;
pub mod lifecycle;
pub mod metrics;
pub mod recovery;
pub mod recovery_driver;
mod runtime;
pub mod scene_pipeline;
pub mod session;
#[cfg(feature = "test-harness")]
pub(crate) mod test_harness;

pub use crate::draw::outcome::{GraphicsFailure, RenderOutcome};
pub use crate::draw::target::{
    GraphicsCapabilities, PresentationMode, RasterPipeline, RenderTarget, ScrollCopy,
    UpdateStrategy,
};
pub use animation::AnimationRegistry;
pub use invalidation::{
    invalidate_paint_handle, Invalidation, InvalidationQueue, InvalidationQueueHandle, ScrollDelta,
};
pub use lifecycle::{begin_frame, end_frame, normalize_strategy};
pub use metrics::{InvalidationSource, RenderMetrics};
pub use recovery::{GraphicsRecovery, RecoveryAction};
pub(crate) use recovery_driver::RebuildRequest;
pub use recovery_driver::{RecoveryDriver, RenderTargetRebuilder};
pub use runtime::Renderer;
pub use scene_pipeline::{FrameRenderInput, FrameRenderOutput, ScenePipeline};
pub use session::RenderSession;
