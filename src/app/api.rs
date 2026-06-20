// ============================================================================
// app/api.rs — Application 层的公共 API 出口
//
// 本文件定义 app 层对外暴露的公共接口。其他层只能通过本文件
// 使用 app 层的功能，禁止直接引用内部模块。
// ============================================================================

pub use crate::app::application::{map_ui_event, App, AppMode, RenderStrategy};
pub use crate::app::cli::Cli;
pub use crate::app::di::Container;
pub use crate::app::window::Window;
