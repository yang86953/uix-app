// ============================================================================
// platform/linux/system_info/mod.rs — Linux system info (ISystemInfo)
// ============================================================================
//
// Reads system information via:
//   - uname(2)   for OS name / version / architecture
//   - /proc/cpuinfo for CPU count
//   - /proc/meminfo  for memory info
//   - gethostname(2) / getlogin_r(3) for hostname / username
//   - /proc/uptime   for uptime
//
// Font discovery via fontconfig lives in the `fonts` submodule.
// ============================================================================

mod fonts;
pub(crate) use fonts::*;

use crate::platform::{ISystemInfo, MemoryInfo, OsInfo};

use std::fs;
use std::io::{self, BufRead};



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
}

// ════════════════════════════════════════════════════════════════════════════
// Internal helpers
// ════════════════════════════════════════════════════════════════════════════

fn probe_os_info() -> OsInfo {
    // SAFETY: uname(2) writes into fixed-size buffers; the syscall always
    // null-terminates, and we only read up to the null terminator.
    let mut utsname: libc::utsname = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::uname(&mut utsname) };
    if ret != 0 {
        return OsInfo {
            name: "Linux (Unknown)".to_string(),
            version: String::new(),
            build: String::new(),
            is_64bit: std::mem::size_of::<usize>() == 8,
        };
    }

    let sysname = cstr_to_string(&utsname.sysname);
    let release = cstr_to_string(&utsname.release);
    let version = cstr_to_string(&utsname.version);
    let machine = cstr_to_string(&utsname.machine);

    let os_name = if sysname == "Linux" {
        // Try to get distro name from /etc/os-release
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
    // Try sysconf first (POSIX)
    let count = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
    if count > 0 {
        return count as u32;
    }

    // Fallback: count "processor" lines in /proc/cpuinfo
    if let Ok(file) = fs::File::open("/proc/cpuinfo") {
        let reader = io::BufReader::new(file);
        return reader
            .lines()
            .filter_map(|line| {
                let l = line.ok()?;
                if l.starts_with("processor") { Some(()) } else { None }
            })
            .count() as u32;
    }

    1
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
    // SAFETY: gethostname(2) writes into a fixed buffer; null-terminated.
    let mut buf = [0i8; 256];
    let ret = unsafe { libc::gethostname(buf.as_mut_ptr(), buf.len()) };
    if ret == 0 {
        cstr_to_string(&buf)
    } else {
        String::new()
    }
}

fn probe_username() -> String {
    // Try $USER first, then $LOGNAME, then /etc/passwd fallback.
    if let Ok(user) = std::env::var("USER") {
        return user;
    }
    if let Ok(user) = std::env::var("LOGNAME") {
        return user;
    }
    // SAFETY: getpwuid_r reads from /etc/passwd; safe with proper buffer.
    unsafe {
        let mut buf = [0i8; 4096];
        let mut pwd: libc::passwd = std::mem::zeroed();
        let mut result: *mut libc::passwd = std::ptr::null_mut();
        let ret = libc::getpwuid_r(
            libc::getuid(),
            &mut pwd,
            buf.as_mut_ptr(),
            buf.len(),
            &mut result,
        );
        if ret == 0 && !result.is_null() && !pwd.pw_name.is_null() {
            let name = std::ffi::CStr::from_ptr(pwd.pw_name);
            return name.to_string_lossy().to_string();
        }
    }
    String::new()
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

// ════════════════════════════════════════════════════════════════════════════
// Utility: convert C fixed-size char array to String
// ════════════════════════════════════════════════════════════════════════════

fn cstr_to_string(arr: &[i8]) -> String {
    let bytes: Vec<u8> = arr
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).to_string()
}
