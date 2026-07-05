//! 应用能力 — 生命周期、主循环、窗口、CLI、DI。

pub mod bridge;
pub mod event_loop;
pub mod shell;
pub mod window;

pub use shell::application::{map_ui_event, App, AppMode};
pub use shell::cli::{Cli, CliArgs};
pub use shell::di::Container;
pub use window::window::Window;
