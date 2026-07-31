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
pub use fonts::*;

use crate::native::traits::system::ISystemInfo;
use crate::native::traits::system::{MemoryInfo, OsInfo};
use crate::native::{Errc, Error, Result};

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
        Ok(match probe_system_default_font() {
            Some(p) => vec![p],
            None => vec![],
        })
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

fn probe_os_info() -> Result<OsInfo> {
    // 安全替代：读取 /proc/sys/kernel/ 下的文本文件
    let sysname = read_proc_text("/proc/sys/kernel/ostype")?;
    let release = read_proc_text("/proc/sys/kernel/osrelease")?;
    let version = read_proc_text("/proc/sys/kernel/version")?;
    let machine = std::process::Command::new("uname")
        .arg("-m")
        .output()
        .map_err(|err| {
            Error::new(
                Errc::IoError,
                format!("LinuxSystemInfo: uname -m failed: {err}"),
            )
        })?
        .stdout;
    let machine = String::from_utf8_lossy(&machine).trim().to_string();

    let os_name = if sysname == "Linux" {
        detect_distro().unwrap_or_else(|| "Linux".to_string())
    } else {
        sysname
    };

    let is_64bit = machine == "x86_64" || machine == "aarch64";

    Ok(OsInfo {
        name: os_name,
        version: release,
        build: version,
        is_64bit,
    })
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
    let content = read_proc_text("/proc/meminfo")?;
    let mut total = 0u64;
    let mut available = 0u64;

    for line in content.lines() {
        if let Some(val) = parse_meminfo_line(line, "MemTotal:") {
            total = val;
        } else if let Some(val) = parse_meminfo_line(line, "MemAvailable:") {
            available = val;
        }
    }

    if total == 0 {
        return Err(Error::new(
            Errc::PlatformError,
            "LinuxSystemInfo: /proc/meminfo missing MemTotal",
        ));
    }

    Ok(MemoryInfo {
        total_bytes: total * 1024, // /proc/meminfo reports in kB
        available_bytes: available * 1024,
        process_working_set: 0,
        process_private_bytes: 0,
    })
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
