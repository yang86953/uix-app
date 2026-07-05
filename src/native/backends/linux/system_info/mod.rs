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

mod fonts;
pub use fonts::*;

use crate::native::traits::system::{MemoryInfo, OsInfo};
use crate::native::ISystemInfo;

use std::fs;

// ════════════════════════════════════════════════════════════════════════════
// LinuxSystemInfo
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct LinuxSystemInfo;

impl LinuxSystemInfo {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxSystemInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ISystemInfo for LinuxSystemInfo {
    fn os_info(&self) -> OsInfo {
        probe_os_info()
    }

    fn cpu_count(&self) -> u32 {
        probe_cpu_count()
    }

    fn memory_info(&self) -> MemoryInfo {
        probe_memory_info()
    }

    fn hostname(&self) -> String {
        probe_hostname()
    }

    fn username(&self) -> String {
        probe_username()
    }

    fn up_time(&self) -> u64 {
        probe_uptime_ms()
    }

    /// 用 fontconfig 查询系统当前默认无衬线字体路径。
    ///
    /// 使用 `fc-match sans-serif:scalable=true` 获取，返回的路径若为
    /// 可变字体（CFF2 VF，fontdue 不兼容），会尝试查同一家族的常规风格。
    fn default_font_path(&self) -> Option<String> {
        probe_font_path_via_fc_match("sans-serif:scalable=true")
            .or_else(|| probe_font_path_via_fc_match("sans-serif"))
    }

    fn default_font_paths(&self) -> Vec<String> {
        match probe_system_default_font() {
            Some(p) => vec![p],
            None => vec![],
        }
    }

    fn probe_cjk_font_path(&self) -> Option<String> {
        probe_cjk_font()
    }

    fn probe_family_font_path(&self, family: &str) -> Option<String> {
        probe_font_path_via_fc_match(family)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Internal helpers
// ════════════════════════════════════════════════════════════════════════════

fn probe_os_info() -> OsInfo {
    // 安全替代：读取 /proc/sys/kernel/ 下的文本文件
    let sysname = std::fs::read_to_string("/proc/sys/kernel/ostype")
        .unwrap_or_default()
        .trim()
        .to_string();
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_default()
        .trim()
        .to_string();
    let version = std::fs::read_to_string("/proc/sys/kernel/version")
        .unwrap_or_default()
        .trim()
        .to_string();
    let machine = std::process::Command::new("uname")
        .arg("-m")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let os_name = if sysname == "Linux" {
        detect_distro().unwrap_or_else(|| "Linux".to_string())
    } else {
        sysname
    };

    let is_64bit = machine == "x86_64" || machine == "aarch64";

    OsInfo {
        name: os_name,
        version: release,
        build: version,
        is_64bit,
    }
}

fn detect_distro() -> Option<String> {
    let content = fs::read_to_string("/etc/os-release").ok()?;
    for line in content.lines() {
        if let Some(name) = line.strip_prefix("PRETTY_NAME=\"") {
            if let Some(end) = name.rfind('"') {
                return Some(name[..end].to_string());
            }
        }
        if let Some(name) = line.strip_prefix("PRETTY_NAME=") {
            return Some(name.to_string());
        }
        if let Some(name) = line.strip_prefix("NAME=\"") {
            if let Some(end) = name.rfind('"') {
                return Some(name[..end].to_string());
            }
        }
        if let Some(name) = line.strip_prefix("NAME=") {
            return Some(name.to_string());
        }
    }
    None
}

fn probe_cpu_count() -> u32 {
    // 安全替代：std::thread::available_parallelism()
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1)
}

fn probe_memory_info() -> MemoryInfo {
    let content = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total = 0u64;
    let mut available = 0u64;

    for line in content.lines() {
        if let Some(val) = parse_meminfo_line(line, "MemTotal:") {
            total = val;
        } else if let Some(val) = parse_meminfo_line(line, "MemAvailable:") {
            available = val;
        }
    }

    MemoryInfo {
        total_bytes: total * 1024, // /proc/meminfo reports in kB
        available_bytes: available * 1024,
        process_working_set: 0,
        process_private_bytes: 0,
    }
}

fn parse_meminfo_line(line: &str, key: &str) -> Option<u64> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix(key) {
        let rest = rest.trim();
        // Split on whitespace, first part is the number
        if let Some(num_str) = rest.split_whitespace().next() {
            return num_str.parse::<u64>().ok();
        }
    }
    None
}

fn probe_hostname() -> String {
    // 安全替代：读取 /proc/sys/kernel/hostname
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn probe_username() -> String {
    // Try $USER first, then $LOGNAME
    if let Ok(user) = std::env::var("USER") {
        return user;
    }
    if let Ok(user) = std::env::var("LOGNAME") {
        return user;
    }
    // 安全替代：读 /etc/passwd 取当前 uid 对应的用户名
    let uid = std::process::Command::new("id")
        .arg("-un")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();
    uid
}

fn probe_uptime_ms() -> u64 {
    // Read /proc/uptime: first field is uptime in seconds (with decimals)
    let content = fs::read_to_string("/proc/uptime").unwrap_or_default();
    if let Some(secs_str) = content.split_whitespace().next() {
        if let Ok(secs) = secs_str.parse::<f64>() {
            return (secs * 1000.0) as u64;
        }
    }
    0
}
