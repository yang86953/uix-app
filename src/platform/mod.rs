//! UIX 的稳定原生平台能力公开面。
//!
//! `crate::native` 保留 OS、窗口和图形实现；应用只通过本模块的 owned
//! 描述值与线程亲和 [`Platform`] 消费独立平台能力。

mod facade;
pub mod graphics;
pub mod hardware;
mod imp;
pub mod services;

pub use facade::Platform;
