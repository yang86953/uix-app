//! UIX 的稳定原生平台能力公开面。
//!
//! `crate::native` 保留 OS、窗口和图形实现；应用只通过本模块的 owned
//! 描述值与线程亲和 [`Platform`] 消费独立平台能力。

mod facade;
pub mod graphics;
pub mod hardware;
mod imp;
pub mod services;
mod ui_thread;

pub use facade::Platform;
// 公开零 cfg 的 UI 线程入口，平台差异由 imp 内部消化。
pub use ui_thread::run_on_ui_thread;
