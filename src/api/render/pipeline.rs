//! 渲染管线协议 — 帧调度、无效化与 RenderSession。

pub use crate::render::engine::RenderOutcome;
pub use crate::render::engine::cpu::software::SoftwareEngine;
pub use crate::render::gpu_engine::GpuEngine;
pub use crate::render::null_engine::NullEngine;
pub use crate::render::pipeline::{
    AnimationRegistry, Invalidation, InvalidationQueue, InvalidationSource, NodeId, RenderMetrics,
    RenderSession, ScrollDelta,
};
