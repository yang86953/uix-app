//! Fake 系统信息 — 完全可配置，支持 &self 访问。

use crate::platform::api::system::ISystemInfo;
use crate::platform::api::system::{MemoryInfo, OsInfo};
use std::cell::Cell;

#[derive(Debug)]
pub struct FakeSystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub os_build: String,
    pub is_64bit: Cell<bool>,
    pub cpu_count: Cell<u32>,
    pub hostname: String,
    pub username: String,
    pub up_time: Cell<u64>,
    pub memory_total: Cell<u64>,
    pub memory_available: Cell<u64>,
    /// MemoryInfo.process_working_set 是 usize
    pub process_working_set: Cell<usize>,
    /// MemoryInfo.process_private_bytes 是 usize
    pub process_private_bytes: Cell<usize>,
    pub default_font: String,
    /// `default_font_path` 调用次数追踪
    pub default_font_calls: std::cell::Cell<usize>,
}

impl FakeSystemInfo {
    pub fn new() -> Self {
        Self {
            os_name: "FakeOS".to_string(),
            os_version: "1.0".to_string(),
            os_build: "build-001".to_string(),
            is_64bit: Cell::new(true),
            cpu_count: Cell::new(4),
            hostname: "fake-host".to_string(),
            username: "user".to_string(),
            up_time: Cell::new(3600),
            memory_total: Cell::new(8 * 1024 * 1024 * 1024), // 8 GB
            memory_available: Cell::new(4 * 1024 * 1024 * 1024), // 4 GB
            process_working_set: Cell::new(256 * 1024 * 1024), // 256 MB
            process_private_bytes: Cell::new(128 * 1024 * 1024), // 128 MB
            default_font: "/usr/share/fonts/NotoSans.ttf".to_string(),
            default_font_calls: std::cell::Cell::new(0),
        }
    }

    /// 清除调用记录（保留其他配置不变）
    pub fn clear_history(&mut self) {
        self.default_font_calls.set(0);
    }
}

impl Default for FakeSystemInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ISystemInfo for FakeSystemInfo {
    fn os_info(&self) -> OsInfo {
        OsInfo {
            name: self.os_name.clone(),
            version: self.os_version.clone(),
            build: self.os_build.clone(),
            is_64bit: self.is_64bit.get(),
        }
    }

    fn cpu_count(&self) -> u32 {
        self.cpu_count.get()
    }

    fn memory_info(&self) -> MemoryInfo {
        MemoryInfo {
            total_bytes: self.memory_total.get(),
            available_bytes: self.memory_available.get(),
            process_working_set: self.process_working_set.get(),
            process_private_bytes: self.process_private_bytes.get(),
        }
    }

    fn hostname(&self) -> String {
        self.hostname.clone()
    }

    fn username(&self) -> String {
        self.username.clone()
    }

    fn up_time(&self) -> u64 {
        self.up_time.get()
    }

    fn default_font_path(&self) -> Option<String> {
        self.default_font_calls
            .set(self.default_font_calls.get() + 1);
        if self.default_font.is_empty() {
            None
        } else {
            Some(self.default_font.clone())
        }
    }
}
