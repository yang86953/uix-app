//! 尚未迁移的 native 文件系统服务协议。

use crate::core::error::{Error, Result};

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

/// 通用状态级别（类型已收口到 platform 公开面，此处保持 native 路径可解析）。
pub(crate) use crate::platform::capabilities::StatusLevel;

pub(crate) trait IFileSystem {
    fn get_special_dir(&self, dir: SpecialDir) -> Result<String>;
    fn executable_path(&self) -> Result<String>;
    fn executable_dir(&self) -> Result<String>;
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error>;
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/native/capabilities/system__special_dir_consistency.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod special_dir_consistency;
