// ============================================================================
// platform/linux/system_info/mod.rs — Linux system info (ISystemInfo)
// ============================================================================
//
// Reads system information via:
//   - /proc/sys/kernel/* for OS name / version / build
//   - /proc/meminfo  for memory info
//   - /proc/sys/kernel/hostname for hostname
//   - $USER / $LOGNAME / id(1) for username
//   - /proc/uptime   for uptime
//
// Font discovery via fontconfig lives in the `fonts` submodule.
// ============================================================================

pub(crate) mod fonts;
pub(crate) use fonts::*;

use crate::native::{Errc, Error, Result};
use crate::platform::system::info::{ISystemInfo, MemoryInfo, OsInfo};

// ════════════════════════════════════════════════════════════════════════════
// LinuxSystemInfo
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub(crate) struct LinuxSystemInfo;

impl LinuxSystemInfo {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for LinuxSystemInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ISystemInfo for LinuxSystemInfo {
    fn os_info(&self) -> Result<OsInfo> {
        probe_os_info()
    }

    fn cpu_count(&self) -> Result<u32> {
        probe_cpu_count()
    }

    fn memory_info(&self) -> Result<MemoryInfo> {
        probe_memory_info()
    }

    fn hostname(&self) -> Result<String> {
        probe_hostname()
    }

    fn username(&self) -> Result<String> {
        probe_username()
    }

    fn up_time(&self) -> Result<u64> {
        probe_uptime_ms()
    }
    fn default_font_paths(&self) -> Result<Vec<String>> {
        // 返回有序候选，由 FontService 以真实栅格探针选择首个可用字体。
        Ok(probe_system_default_font_paths())
    }

    fn probe_cjk_font_path(&self) -> Option<String> {
        probe_cjk_font()
    }

    // 返回 fontconfig 的有序多候选列表，允许字体服务跳过实际缺少 CJK 字形的首项。
    fn probe_cjk_font_paths(&self) -> Vec<String> {
        // 平台 Component 只负责发现路径，真实字形覆盖仍由 FontService 验证。
        probe_cjk_font_paths()
    }

    fn probe_family_font_path(&self, family: &str) -> Option<String> {
        probe_font_path_via_fc_match(family)
    }

    fn default_font_sources(&self) -> Result<Vec<crate::platform::services::SystemFontSource>> {
        Ok(probe_system_default_font_sources())
    }

    fn probe_cjk_font_sources(&self) -> Vec<crate::platform::services::SystemFontSource> {
        probe_cjk_font_sources()
    }

    fn probe_family_font_source(
        &self,
        family: &str,
    ) -> Option<crate::platform::services::SystemFontSource> {
        probe_font_source_via_fc_match(family)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Internal helpers
// ════════════════════════════════════════════════════════════════════════════

fn probe_os_info() -> Result<OsInfo> {
    // OS 采集唯一事实源是公开硬件 Provider；本端口只做内部值映射。
    crate::platform::capabilities::providers::os_info().map(OsInfo::from_hardware)
}

fn read_proc_text(path: &str) -> Result<String> {
    std::fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .map_err(|err| {
            Error::new(
                Errc::IoError,
                format!("LinuxSystemInfo: cannot read {path}: {err}"),
            )
        })
}

fn probe_cpu_count() -> Result<u32> {
    // 安全替代：std::thread::available_parallelism()
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .map_err(|err| {
            Error::new(
                Errc::PlatformError,
                format!("LinuxSystemInfo: available_parallelism failed: {err}"),
            )
        })
}

fn probe_memory_info() -> Result<MemoryInfo> {
    // 内存采集唯一事实源是公开硬件 Provider；本端口只做内部值映射。
    crate::platform::capabilities::providers::memory_info().map(MemoryInfo::from_hardware)
}

fn probe_hostname() -> Result<String> {
    // 安全替代：读取 /proc/sys/kernel/hostname
    read_proc_text("/proc/sys/kernel/hostname")
}

fn probe_username() -> Result<String> {
    // Try $USER first, then $LOGNAME
    if let Ok(user) = std::env::var("USER") {
        return Ok(user);
    }
    if let Ok(user) = std::env::var("LOGNAME") {
        return Ok(user);
    }
    // 安全替代：读 /etc/passwd 取当前 uid 对应的用户名
    let uid = std::process::Command::new("id")
        .arg("-un")
        .output()
        .map_err(|err| {
            Error::new(
                Errc::IoError,
                format!("LinuxSystemInfo: id -un failed: {err}"),
            )
        })?
        .stdout;
    let uid = String::from_utf8_lossy(&uid).trim().to_string();
    if uid.is_empty() {
        Err(Error::new(
            Errc::NotFound,
            "LinuxSystemInfo: cannot determine username",
        ))
    } else {
        Ok(uid)
    }
}

fn probe_uptime_ms() -> Result<u64> {
    // Read /proc/uptime: first field is uptime in seconds (with decimals)
    let content = read_proc_text("/proc/uptime")?;
    if let Some(secs_str) = content.split_whitespace().next() {
        if let Ok(secs) = secs_str.parse::<f64>() {
            return Ok((secs * 1000.0) as u64);
        }
    }
    Err(Error::new(
        Errc::FormatError,
        "LinuxSystemInfo: /proc/uptime has unexpected format",
    ))
}
