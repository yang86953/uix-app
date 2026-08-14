//! UIX 的稳定原生平台能力公开面。
//!
//! `crate::native` 保留 OS、窗口和图形实现；应用只通过本模块的 owned
//! 描述值与线程亲和 [`Platform`] 消费独立平台能力。

// 平台公开面的输入契约、能力状态与呈现装配窄 API（ui/draw/app 收口入口）。
pub mod capabilities;
mod facade;
pub mod graphics;
pub mod hardware;
mod imp;
pub mod presentation;
pub mod services;
mod ui_thread;
pub mod windowing;

pub use facade::Platform;
// 公开零 cfg 的 UI 线程入口，平台差异由 imp 内部消化。
pub use ui_thread::run_on_ui_thread;
