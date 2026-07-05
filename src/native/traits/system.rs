//! 系统服务协议 — 文件、对话框、通知、定时器、控制台与系统信息。

use crate::core::error::{Error, Result};

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
    fn open(&mut self, title: &str, filters: &str) -> Vec<String>;
    fn save(&mut self, title: &str, filters: &str) -> String;
    fn open_folder(&mut self, title: &str) -> String;
}

pub trait IFileSystem {
    fn get_special_dir(&self, dir: SpecialDir) -> String;
    fn executable_path(&self) -> String;
    fn executable_dir(&self) -> String;
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error>;
}

pub trait INotification {
    fn show(&mut self, title: &str, message: &str);
}

pub trait ITimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> u32;
    fn clear(&mut self, id: u32);
}

pub trait ISystemInfo {
    fn os_info(&self) -> OsInfo;
    fn cpu_count(&self) -> u32;
    fn memory_info(&self) -> MemoryInfo;
    fn hostname(&self) -> String;
    fn username(&self) -> String;
    fn up_time(&self) -> u64;
    fn default_font_paths(&self) -> Vec<String>;
    fn probe_cjk_font_path(&self) -> Option<String> {
        None
    }
    fn probe_family_font_path(&self, _family: &str) -> Option<String> {
        None
    }
    fn scan_fallback_font_path(&self) -> Option<String> {
        None
    }
    fn process_memory(&self) -> (usize, usize) {
        (0, 0)
    }
}

pub trait IConsole {
    fn write(&mut self, text: &str);
    fn write_line(&mut self, text: &str);
    fn set_color(&mut self, color: ConsoleColor);
    fn reset_color(&mut self);
    fn show_terminal_cursor(&mut self, visible: bool);
    fn set_terminal_title(&mut self, title: &str);
    fn capabilities(&self) -> TerminalCapabilities;
}
