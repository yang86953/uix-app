//! 反馈组件：弹层、提示与加载状态。

pub mod alert;
// 声明租约组件与关闭事实共享反馈 capability。
pub mod declaration;
pub mod drawer;
pub mod message;
pub mod modal;
pub mod notification;
pub mod popconfirm;
pub mod popover;
pub mod progress;
// 集中验证 ProgressBar fraction、模式、动画与可观察归一化契约。
#[cfg(test)]
mod progress_tests;
pub mod spin;
pub(crate) mod toast_motion;
pub mod tooltip;

pub use alert::*;
pub use declaration::*;
pub use drawer::*;
pub use message::*;
pub use modal::*;
pub use notification::*;
pub use popconfirm::*;
pub use popover::*;
pub use progress::*;
pub use spin::*;
pub use tooltip::*;
