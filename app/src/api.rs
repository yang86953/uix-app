//! # uix-app 稳定公开 API
//!
//! app 层提供应用入口和窗口生命周期管理。
//! 本层无 trait，所有公开类型直接 re-export。

pub use crate::application::{map_ui_event, App, AppMode};
pub use crate::cli::Cli;
pub use crate::di::Container;
pub use crate::window::Window;
