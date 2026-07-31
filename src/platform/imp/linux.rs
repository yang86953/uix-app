use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::core::{Errc, Error, Result};
use crate::native::backends::linux::platform::LinuxPlatform;
use crate::native::platform::Platform as NativePlatform;
use crate::platform::hardware::{DisplayInfo, MemoryInfo, OsInfo};
use crate::platform::services::{SpecialDir, SystemNotification};

pub(crate) struct State;

impl State {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self)
    }
}

pub(crate) fn is_main_thread() -> Result<bool> {
    // Linux defines the thread-group leader's TID to be the process ID.
    // SAFETY: getpid/syscall have no pointer arguments and no resource ownership.
    let process_id = unsafe { libc::getpid() } as libc::c_long;
    let thread_id = unsafe { libc::syscall(libc::SYS_gettid) };
    if thread_id < 0 {
        return Err(Error::new(
            Errc::PlatformError,
            format!(
                "Platform::new: gettid failed: {}",
                std::io::Error::last_os_error()
            ),
        ));
    }
    Ok(thread_id == process_id)
}

pub(crate) fn os_info() -> Result<OsInfo> {
    let release = read_trimmed("/proc/sys/kernel/osrelease").ok();
    let build = read_trimmed("/proc/sys/kernel/version").ok();
    let name = os_release_value("PRETTY_NAME")
        .or_else(|| os_release_value("NAME"))
        .unwrap_or_else(|| "Linux".to_owned());
    if name.trim().is_empty() {
        return Err(Error::new(
            Errc::PlatformError,
            "Platform::os_info: Linux returned an empty system name",
        ));
    }
    Ok(OsInfo::new(
        name,
        release.and_then(non_empty),
        build.and_then(non_empty),
    ))
}

pub(crate) fn cpu_metadata() -> (Option<String>, Option<String>) {
    let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") else {
        return (None, None);
    };
    let vendor = cpuinfo_value(&content, &["vendor_id", "CPU implementer"]);
    let model = cpuinfo_value(&content, &["model name", "Hardware", "Processor"]);
    (vendor, model)
}

pub(crate) fn memory_info() -> Result<MemoryInfo> {
    let content = std::fs::read_to_string("/proc/meminfo").map_err(|source| {
        Error::new(
            io_code(&source),
            format!("Platform::memory_info: cannot read /proc/meminfo: {source}"),
        )
    })?;
    let total = meminfo_bytes(&content, "MemTotal").ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "Platform::memory_info: /proc/meminfo has no MemTotal",
        )
    })?;
    let available = meminfo_bytes(&content, "MemAvailable").or_else(|| {
        ["MemFree", "Buffers", "Cached"]
            .into_iter()
            .try_fold(0u64, |sum, key| {
                sum.checked_add(meminfo_bytes(&content, key)?)
            })
    });
    let available = available.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "Platform::memory_info: /proc/meminfo has no usable available-memory fields",
        )
    })?;
    if total == 0 || available > total {
        return Err(Error::new(
            Errc::PlatformError,
            "Platform::memory_info: Linux returned invalid physical-memory totals",
        ));
    }
    Ok(MemoryInfo::new(total, available))
}

pub(crate) fn displays() -> Result<Box<[DisplayInfo]>> {
    let platform = LinuxPlatform::new().map_err(|error| {
        Error::new(
            error.code(),
            format!("Platform::displays: {}", error.message()),
        )
        .with_source(error)
    })?;
    let display = platform.display();
    let count = usize::try_from(display.count()?).map_err(|_| {
        Error::new(
            Errc::PlatformError,
            "Platform::displays: Wayland returned a negative display count",
        )
    })?;
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        let index = i32::try_from(index).map_err(|_| {
            Error::new(
                Errc::PlatformError,
                "Platform::displays: display index exceeds i32",
            )
        })?;
        let value = display.info(index)?;
        values.push(DisplayInfo::new(
            value.bounds,
            value.dpi_scale,
            value.is_primary,
            None,
            None,
        ));
    }
    Ok(values.into_boxed_slice())
}

pub(crate) fn special_dir(directory: SpecialDir) -> Result<PathBuf> {
    let home = home_dir()?;
    let path = match directory {
        SpecialDir::Home => home,
        SpecialDir::AppData => env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")),
        SpecialDir::LocalAppData => {
            env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local").join("share"))
        }
        SpecialDir::Documents => {
            xdg_user_dir("DOCUMENTS", &home).unwrap_or_else(|| home.join("Documents"))
        }
        SpecialDir::Desktop => {
            xdg_user_dir("DESKTOP", &home).unwrap_or_else(|| home.join("Desktop"))
        }
        SpecialDir::Downloads => {
            xdg_user_dir("DOWNLOAD", &home).unwrap_or_else(|| home.join("Downloads"))
        }
    };
    if path.as_os_str().is_empty() {
        Err(Error::new(
            Errc::NotFound,
            "Platform::special_dir: Linux returned an empty directory path",
        ))
    } else {
        Ok(path)
    }
}

pub(crate) fn show_notification(notification: &SystemNotification) -> Result<()> {
    let status = Command::new("notify-send")
        .arg("--app-name")
        .arg("UIX")
        .arg(notification.title())
        .arg(notification.message())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|source| {
            let code = match source.kind() {
                std::io::ErrorKind::NotFound => Errc::NotImplemented,
                std::io::ErrorKind::PermissionDenied => Errc::PermissionDenied,
                _ => Errc::PlatformError,
            };
            Error::new(
                code,
                format!("Platform::show_notification: cannot launch notify-send: {source}"),
            )
        })?;
    if status.success() {
        return Ok(());
    }
    Err(Error::new(
        Errc::PlatformError,
        format!("Platform::show_notification: notify-send exited with {status}"),
    ))
}

fn home_dir() -> Result<PathBuf> {
    env_path("HOME").ok_or_else(|| {
        Error::new(
            Errc::NotFound,
            "Platform::special_dir: HOME is not available",
        )
    })
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn xdg_user_dir(key: &str, home: &Path) -> Option<PathBuf> {
    let config_home = env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
    let content = std::fs::read_to_string(config_home.join("user-dirs.dirs")).ok()?;
    let prefix = format!("XDG_{key}_DIR=");
    let encoded = content
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&prefix))?
        .trim();
    let value = encoded
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(encoded);
    if value == "$HOME" {
        return Some(home.to_path_buf());
    }
    if let Some(suffix) = value.strip_prefix("$HOME/") {
        return Some(home.join(suffix));
    }
    let path = PathBuf::from(OsString::from(value));
    path.is_absolute().then_some(path)
}

fn read_trimmed(path: &str) -> Result<String> {
    std::fs::read_to_string(path)
        .map(|value| value.trim().to_owned())
        .map_err(|source| {
            Error::new(
                io_code(&source),
                format!("Platform::os_info: cannot read {path}: {source}"),
            )
        })
}

fn os_release_value(key: &str) -> Option<String> {
    let content = std::fs::read_to_string("/etc/os-release").ok()?;
    let prefix = format!("{key}=");
    let encoded = content
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))?;
    let value = encoded
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(encoded)
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    non_empty(value)
}

fn cpuinfo_value(content: &str, keys: &[&str]) -> Option<String> {
    for line in content.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if keys.iter().any(|candidate| key.trim() == *candidate) {
            if let Some(value) = non_empty(value.trim().to_owned()) {
                return Some(value);
            }
        }
    }
    None
}

fn meminfo_bytes(content: &str, key: &str) -> Option<u64> {
    let prefix = format!("{key}:");
    let value = content.lines().find_map(|line| {
        line.strip_prefix(&prefix)?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
    })?;
    value.checked_mul(1_024)
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn io_code(error: &std::io::Error) -> Errc {
    match error.kind() {
        std::io::ErrorKind::NotFound => Errc::NotFound,
        std::io::ErrorKind::PermissionDenied => Errc::PermissionDenied,
        _ => Errc::PlatformError,
    }
}
