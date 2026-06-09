// ============================================================================
// platform/file_system.rs — 文件系统抽象接口
// ============================================================================

use crate::diag::Error;

// ════════════════════════════════════════════════════════════════════════════
// 特殊目录枚举
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
// IFileSystem — 文件系统能力接口
// ════════════════════════════════════════════════════════════════════════════

pub trait IFileSystem {
    /// Get the path to a special directory.
    fn get_special_dir(&self, dir: SpecialDir) -> String;

    /// Get the path of the current executable.
    fn executable_path(&self) -> String;

    /// Get the directory of the current executable.
    fn executable_dir(&self) -> String;

    /// Read the entire contents of a file into a byte vector.
    /// Returns `Err` if the file cannot be read (not found, permission denied, etc.).
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error>;
}
