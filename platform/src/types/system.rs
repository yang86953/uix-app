// ============================================================================
// platform/types/system.rs — 系统级数据类型
//
// IFileDialog / IFileSystem / INotification / ITimer / ISystemInfo
// 已迁移至 crate::api::traits。
// ============================================================================

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
