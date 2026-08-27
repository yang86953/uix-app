//! 单一 Renderer、帧生命周期与调度状态。

pub mod animation;
pub(crate) mod bootstrap;
pub mod invalidation;
pub mod lifecycle;
pub mod metrics;
pub mod recovery;
pub mod recovery_driver;
mod runtime;
pub mod scene_pipeline;
pub mod session;
#[cfg(any(feature = "test-harness", feature = "agent-control"))]
// 将测试支撑实现统一存放在根 tests 目录。
#[path = "../../../tests/support/draw/renderer/test_harness.rs"]
pub(crate) mod test_harness;

pub use crate::draw::outcome::{GraphicsFailure, RenderOutcome};
pub use crate::draw::target::{
    GraphicsCapabilities, PresentationMode, RasterPipeline, RenderTarget, ScrollCopy,
    UpdateStrategy,
};
pub use animation::AnimationRegistry;
pub use invalidation::{
    Invalidation, InvalidationQueue, InvalidationQueueHandle, ScrollDelta, invalidate_paint_handle,
};
pub use lifecycle::{begin_frame, end_frame, normalize_strategy};
pub use metrics::{InvalidationSource, RenderMetrics};
pub use recovery::{GraphicsRecovery, GraphicsRecoveryAction};
pub(crate) use recovery_driver::RebuildRequest;
pub use recovery_driver::{RecoveryDriver, RenderTargetRebuilder};
pub use runtime::Renderer;
// 公开一次性票据类型，应用测试无需访问 renderer 私有控制信号。
pub use scene_pipeline::{FrameRenderInput, FrameRenderOutput, ScenePipeline};
pub use session::RenderSession;
#[cfg(any(feature = "test-harness", feature = "agent-control"))]
pub use test_harness::SurfaceReadbackTicket;
