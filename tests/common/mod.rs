//! 集成测试共享工具。

#![allow(dead_code)]

/// 解析日志级别（测试辅助）。
pub fn test_log_level_from_env() -> uix::core::log::Level {
    std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| match s.to_lowercase().as_str() {
            "trace" => Some(uix::core::log::Level::Trace),
            "debug" => Some(uix::core::log::Level::Debug),
            "warn" => Some(uix::core::log::Level::Warn),
            "error" => Some(uix::core::log::Level::Error),
            _ => None,
        })
        .unwrap_or(uix::core::log::Level::Info)
}
