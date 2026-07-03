//! 帧调度管线。

pub mod frame;
pub mod session;

pub use frame::{begin_frame, end_frame, normalize_strategy};
pub use session::RenderSession;
