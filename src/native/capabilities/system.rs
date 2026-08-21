//! 尚未迁移的 native 系统服务协议 — 文件与系统信息。

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

/// 通用状态级别（类型已收口到 platform 公开面，此处保持 native 路径可解析）。
pub(crate) use crate::platform::capabilities::StatusLevel;

pub(crate) trait IFileSystem {
    fn get_special_dir(&self, dir: SpecialDir) -> Result<String>;
    fn executable_path(&self) -> Result<String>;
    fn executable_dir(&self) -> Result<String>;
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error>;
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

// 将完整 OS 系统信息实现收窄为 Drawing 只允许消费的字体发现协议。
impl<T> crate::platform::services::FontSystemInfo for T
where
    T: ISystemInfo + ?Sized,
{
    fn default_font_paths(&self) -> Result<Vec<String>> {
        ISystemInfo::default_font_paths(self)
    }

    fn probe_cjk_font_paths(&self) -> Vec<String> {
        ISystemInfo::probe_cjk_font_paths(self)
    }

    fn probe_family_font_path(&self, family: &str) -> Option<String> {
        ISystemInfo::probe_family_font_path(self, family)
    }

    fn scan_fallback_font_path(&self) -> Option<String> {
        ISystemInfo::scan_fallback_font_path(self)
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/native/capabilities/system__special_dir_consistency.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod special_dir_consistency;
