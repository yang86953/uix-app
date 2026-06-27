//! UIX Services — Business service layer.
//!
//! 包含业务服务与诊断基础设施（日志、错误收集、恢复策略、崩溃处理）。

pub mod collector;
pub mod fatal;
pub mod file_service;
pub mod log;
pub mod log_format;
pub mod middleware;
pub mod notification_service;
pub mod recovery;
pub mod recovery_policy;
pub mod settings;

mod api;
pub use api::*;
