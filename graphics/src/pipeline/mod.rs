//! 帧调度管线。

pub mod animation_registry;
pub mod frame;
pub mod invalidation;
pub mod metrics;
pub mod session;

pub use animation_registry::AnimationRegistry;
pub use frame::{begin_frame, end_frame, normalize_strategy};
pub use invalidation::{Invalidation, InvalidationQueue, NodeId, ScrollDelta};
pub use metrics::{InvalidationSource, RenderMetrics};
pub use session::RenderSession;
