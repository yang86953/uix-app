//! 帧调度管线。

pub mod frame;
pub mod metrics;
pub mod session;

pub use frame::{begin_frame, end_frame, normalize_strategy};
pub use metrics::{InvalidationSource, RenderMetrics};
pub use session::RenderSession;
