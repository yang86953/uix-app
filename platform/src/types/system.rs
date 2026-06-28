// ============================================================================
// platform/types/system.rs — 系统级 API 契约与数据类型
//
// 文件对话框、文件系统、通知、定时器、系统信息。
// ============================================================================

use crate::error::Error;

// ════════════════════════════════════════════════════════════════════════════
// IFileDialog — 文件对话框
// ════════════════════════════════════════════════════════════════════════════

pub trait IFileDialog {
    fn open(&mut self, title: &str, filters: &str) -> Vec<String>;
    fn save(&mut self, title: &str, filters: &str) -> String;
    fn open_folder(&mut self, title: &str) -> String;
}

// ════════════════════════════════════════════════════════════════════════════
// IFileSystem — 文件系统
// ════════════════════════════════════════════════════════════════════════════

pub trait IFileSystem {
    fn get_special_dir(&self, dir: SpecialDir) -> String;
    fn executable_path(&self) -> String;
    fn executable_dir(&self) -> String;
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error>;
}

// ════════════════════════════════════════════════════════════════════════════
// INotification — 系统通知
// ════════════════════════════════════════════════════════════════════════════

pub trait INotification {
    /// 显示系统通知
    fn show(&mut self, title: &str, message: &str);
}

// ════════════════════════════════════════════════════════════════════════════
// ITimer — 定时器
// ════════════════════════════════════════════════════════════════════════════

pub trait ITimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> u32;
    fn clear(&mut self, id: u32);
}

// ════════════════════════════════════════════════════════════════════════════
// ISystemInfo — 系统信息
// ════════════════════════════════════════════════════════════════════════════

pub trait ISystemInfo {
    fn os_info(&self) -> OsInfo;
    fn cpu_count(&self) -> u32;
    fn memory_info(&self) -> MemoryInfo;
    fn hostname(&self) -> String;
    fn username(&self) -> String;
    fn up_time(&self) -> u64;
    /// 返回系统当前默认字体文件路径（如有）。
    fn default_font_path(&self) -> Option<String>;
    /// 返回系统默认字体文件路径列表（用于回退链）。
    fn default_font_paths(&self) -> Vec<String> {
        self.default_font_path().into_iter().collect()
    }
    /// 探测 CJK 回退字体路径（不支持的平台返回 None）。
    fn probe_cjk_font_path(&self) -> Option<String> { None }
    /// 通过字体族名称查找字体路径（不支持的平台返回 None）。
    fn probe_family_font_path(&self, _family: &str) -> Option<String> { None }
    /// 获取进程内存使用信息。返回 (工作集字节, 私有字节)。
    /// 不支持时返回 (0, 0)。
    fn process_memory(&self) -> (usize, usize) { (0, 0) }
}

// ════════════════════════════════════════════════════════════════════════════
// 文件系统（file_system 子系统）
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
// 系统信息（system_info 子系统）
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
