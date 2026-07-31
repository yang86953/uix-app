//! 系统服务协议 — 文件、对话框、通知、定时器、控制台与系统信息。

use crate::core::error::{Errc, Error, Result};

// ════════════════════════════════════════════════════════════════════════════
// 文件系统
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpecialDir {
    Home,
    Temp,
    AppData,
    LocalAppData,
    Documents,
    Desktop,
    Downloads,
    Current,
    Executable,
}

// ════════════════════════════════════════════════════════════════════════════
// 系统信息
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub process_working_set: usize,
    pub process_private_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct OsInfo {
    pub name: String,
    pub version: String,
    pub build: String,
    pub is_64bit: bool,
}

// ════════════════════════════════════════════════════════════════════════════
// 终端颜色 / 能力
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConsoleColor {
    Default = 0,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalCapabilities {
    pub has_color: bool,
    pub has_raw_mode: bool,
    pub has_cursor_control: bool,
}

/// 通用状态级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Success,
    Info,
    Warning,
    Error,
}

pub trait IFileDialog {
    /// 打开文件选择对话框。`Ok(None)` 表示用户取消；`Err` 表示对话框本身失败。
    fn open(&mut self, title: &str, filters: &str) -> Result<Option<Vec<String>>>;
    /// 打开保存对话框。`Ok(None)` 表示用户取消；`Err` 表示对话框本身失败。
    fn save(&mut self, title: &str, filters: &str) -> Result<Option<String>>;
    /// 打开目录选择对话框。`Ok(None)` 表示用户取消；`Err` 表示对话框本身失败。
    fn open_folder(&mut self, title: &str) -> Result<Option<String>>;
}

pub trait IFileSystem {
    fn get_special_dir(&self, dir: SpecialDir) -> Result<String>;
    fn executable_path(&self) -> Result<String>;
    fn executable_dir(&self) -> Result<String>;
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error>;
}

pub trait INotification {
    /// 显示系统通知。失败时返回 typed error（如通知区域不可用、notify-send 缺失）。
    fn show(&mut self, title: &str, message: &str) -> Result<()>;
}

pub trait ITimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> Result<u32>;
    fn clear(&mut self, id: u32) -> Result<()>;
}

pub trait ISystemInfo {
    fn os_info(&self) -> Result<OsInfo>;
    fn cpu_count(&self) -> Result<u32>;
    fn memory_info(&self) -> Result<MemoryInfo>;
    fn hostname(&self) -> Result<String>;
    fn username(&self) -> Result<String>;
    fn up_time(&self) -> Result<u64>;
    fn default_font_paths(&self) -> Result<Vec<String>>;
    fn probe_cjk_font_path(&self) -> Option<String> {
        None
    }
    /// 有序 CJK 候选；启动至多成功装载一枚。默认包装 [`Self::probe_cjk_font_path`]。
    fn probe_cjk_font_paths(&self) -> Vec<String> {
        self.probe_cjk_font_path().into_iter().collect()
    }
    fn probe_family_font_path(&self, _family: &str) -> Option<String> {
        None
    }
    fn scan_fallback_font_path(&self) -> Option<String> {
        None
    }
    fn process_memory(&self) -> Result<(usize, usize)> {
        Err(Error::new(
            Errc::NotImplemented,
            "ISystemInfo::process_memory: not provided by this backend",
        ))
    }
}

pub trait IConsole {
    fn write(&mut self, text: &str) -> Result<()>;
    fn write_line(&mut self, text: &str) -> Result<()>;
    fn set_color(&mut self, color: ConsoleColor) -> Result<()>;
    fn reset_color(&mut self) -> Result<()>;
    fn show_terminal_cursor(&mut self, visible: bool) -> Result<()>;
    fn set_terminal_title(&mut self, title: &str) -> Result<()>;
    fn capabilities(&self) -> TerminalCapabilities;
}
