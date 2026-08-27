//! Diagnostics System 私有的调试模式状态与关联身份。

use std::ffi::OsStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// 运行时调试状态的唯一所有者。
///
/// 关闭路径只读取一个原子布尔值；关联身份仅在调用方确认需要记录时分配。
pub(super) struct DebugModule {
    enabled: AtomicBool,
    next_correlation_id: AtomicU64,
}

impl DebugModule {
    pub(super) const fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            next_correlation_id: AtomicU64::new(1),
        }
    }

    #[inline(always)]
    pub(super) fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub(super) fn set_enabled(&self, enabled: bool) -> bool {
        self.enabled.swap(enabled, Ordering::Relaxed) != enabled
    }

    pub(super) fn next_correlation_id(&self) -> u64 {
        let id = self.next_correlation_id.fetch_add(1, Ordering::Relaxed);
        // 零保留给“没有关联身份”的结构化日志字段。
        if id == 0 {
            self.next_correlation_id.fetch_add(1, Ordering::Relaxed)
        } else {
            id
        }
    }
}

/// 读取组合根使用的 `UIX_DEBUG` 覆写。
///
/// 缺少变量时返回 `None`，非法值返回 `Some(Err(()))`，避免把任意非空字符串
/// （尤其是常见的 `0`）误判为启用。
pub(crate) fn debug_mode_from_env() -> Option<Result<bool, ()>> {
    std::env::var_os("UIX_DEBUG").map(|value| parse_debug_switch(&value))
}

fn parse_debug_switch(value: &OsStr) -> Result<bool, ()> {
    let normalized = value.to_str().ok_or(())?.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "" | "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(()),
    }
}
