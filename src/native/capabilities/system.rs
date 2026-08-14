//! 系统服务协议 — 文件、对话框、通知、定时器、控制台与系统信息。

use crate::core::error::{Errc, Error, Result};

// ════════════════════════════════════════════════════════════════════════════
// 文件系统
// ════════════════════════════════════════════════════════════════════════════

// 与 [`crate::platform::services::SpecialDir`] 保持共享语义：platform 版是公开
// 权威（6 个 OS-known 目录），本类型是 native 内部文件系统契约，额外提供
// Temp/Current/Executable 三个内部变体（后两者由 FileSystemCore 直接处理）。
// 两处定义通过下方 `From` 转换与 `special_dir_consistency` 测试保持同步，
// 增删共享变体时必须同时修改两处。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum SpecialDir {
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

// platform 公开的 6 个 OS-known 目录是 native 9 变体的子集，可无损转换。
impl From<crate::platform::services::SpecialDir> for SpecialDir {
    fn from(dir: crate::platform::services::SpecialDir) -> Self {
        match dir {
            crate::platform::services::SpecialDir::Home => Self::Home,
            crate::platform::services::SpecialDir::AppData => Self::AppData,
            crate::platform::services::SpecialDir::LocalAppData => Self::LocalAppData,
            crate::platform::services::SpecialDir::Documents => Self::Documents,
            crate::platform::services::SpecialDir::Desktop => Self::Desktop,
            crate::platform::services::SpecialDir::Downloads => Self::Downloads,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 系统信息
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub(crate) struct MemoryInfo {
    pub(crate) total_bytes: u64,
    pub(crate) available_bytes: u64,
    pub(crate) process_working_set: usize,
    pub(crate) process_private_bytes: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct OsInfo {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) build: String,
    pub(crate) is_64bit: bool,
}

// ════════════════════════════════════════════════════════════════════════════
// 终端颜色 / 能力
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ConsoleColor {
    Default = 0,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TerminalCapabilities {
    pub(crate) has_color: bool,
    pub(crate) has_raw_mode: bool,
    pub(crate) has_cursor_control: bool,
}

/// 通用状态级别（类型已收口到 platform 公开面，此处保持 native 路径可解析）。
pub(crate) use crate::platform::capabilities::StatusLevel;

pub(crate) trait IFileDialog {
    /// 打开文件选择对话框。`Ok(None)` 表示用户取消；`Err` 表示对话框本身失败。
    fn open(&mut self, title: &str, filters: &str) -> Result<Option<Vec<String>>>;
    /// 打开保存对话框。`Ok(None)` 表示用户取消；`Err` 表示对话框本身失败。
    fn save(&mut self, title: &str, filters: &str) -> Result<Option<String>>;
    /// 打开目录选择对话框。`Ok(None)` 表示用户取消；`Err` 表示对话框本身失败。
    fn open_folder(&mut self, title: &str) -> Result<Option<String>>;
}

pub(crate) trait IFileSystem {
    fn get_special_dir(&self, dir: SpecialDir) -> Result<String>;
    fn executable_path(&self) -> Result<String>;
    fn executable_dir(&self) -> Result<String>;
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error>;
}

pub(crate) trait INotification {
    /// 显示系统通知。失败时返回 typed error（如通知区域不可用、notify-send 缺失）。
    fn show(&mut self, title: &str, message: &str) -> Result<()>;
}

pub(crate) trait ITimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> Result<u32>;
    fn clear(&mut self, id: u32) -> Result<()>;
}

pub(crate) trait ISystemInfo {
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

pub(crate) trait IConsole {
    fn write(&mut self, text: &str) -> Result<()>;
    fn write_line(&mut self, text: &str) -> Result<()>;
    fn set_color(&mut self, color: ConsoleColor) -> Result<()>;
    fn reset_color(&mut self) -> Result<()>;
    fn show_terminal_cursor(&mut self, visible: bool) -> Result<()>;
    fn set_terminal_title(&mut self, title: &str) -> Result<()>;
    fn capabilities(&self) -> TerminalCapabilities;
}

#[cfg(test)]
mod special_dir_consistency {
    use super::SpecialDir;

    // 断言 platform 公开的 6 个 OS-known 目录逐一映射到 native 契约变体；
    // 任一侧增删共享变体都会使本测试失败，提醒同步两处定义。
    #[test]
    fn platform_variants_are_a_subset_of_native() {
        let pairs = [
            (crate::platform::services::SpecialDir::Home, SpecialDir::Home),
            (
                crate::platform::services::SpecialDir::AppData,
                SpecialDir::AppData,
            ),
            (
                crate::platform::services::SpecialDir::LocalAppData,
                SpecialDir::LocalAppData,
            ),
            (
                crate::platform::services::SpecialDir::Documents,
                SpecialDir::Documents,
            ),
            (
                crate::platform::services::SpecialDir::Desktop,
                SpecialDir::Desktop,
            ),
            (
                crate::platform::services::SpecialDir::Downloads,
                SpecialDir::Downloads,
            ),
        ];
        for (platform, native) in pairs {
            assert_eq!(SpecialDir::from(platform), native);
        }
    }
}
