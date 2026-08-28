//! Platform System 的中立系统信息合同。

use crate::core::error::{Errc, Error, Result};

/// 系统与当前进程的内存使用事实。
#[derive(Debug, Clone)]
pub(crate) struct MemoryInfo {
    pub(crate) total_bytes: u64,
    pub(crate) available_bytes: u64,
    pub(crate) process_working_set: usize,
    pub(crate) process_private_bytes: usize,
}

impl MemoryInfo {
    /// 以公开硬件 Provider 的采集结果为唯一事实源构造系统内存事实；
    /// 进程级字段由后端经独立端口补齐。
    pub(crate) fn from_hardware(value: crate::platform::hardware::MemoryInfo) -> Self {
        Self {
            total_bytes: value.total_bytes(),
            available_bytes: value.available_bytes(),
            process_working_set: 0,
            process_private_bytes: 0,
        }
    }
}

/// 操作系统身份与位数事实。
#[derive(Debug, Clone)]
pub(crate) struct OsInfo {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) build: String,
    pub(crate) is_64bit: bool,
}

impl OsInfo {
    /// 以公开硬件 Provider 的采集结果为唯一事实源构造内部系统身份。
    ///
    /// OS 采集（版本探测、发行版识别）只允许存在于 Provider 一处；
    /// 进程位数由编译目标宽度决定，与宿主内核架构解耦。
    pub(crate) fn from_hardware(value: crate::platform::hardware::OsInfo) -> Self {
        Self {
            name: value.name().to_string(),
            version: value.version().unwrap_or_default().to_string(),
            build: value.build().unwrap_or_default().to_string(),
            is_64bit: cfg!(target_pointer_width = "64"),
        }
    }
}

/// Platform 根持有的系统信息与原生字体发现端口。
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

// 将完整系统信息实现收窄为 Drawing 只允许消费的字体发现协议。
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
