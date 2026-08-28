use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::core::{Errc, Error, Result};
use crate::platform::hardware::{DisplayInfo, MemoryInfo, OsInfo};
use crate::platform::platform::PlatformSystem;
use crate::platform::{PendingNativeOptions, create_platform_with_pending};
// 引入跨平台通知身份与能力状态契约。
use crate::platform::services::{
    AppUserModelId, FileDialogFilter, SpecialDir, SystemNotification, SystemNotificationCapability,
};

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
    let pending_native = PendingNativeOptions::new(crate::diagnostics::PendingFailureQueue::new());
    let platform: Box<dyn PlatformSystem> =
        create_platform_with_pending(pending_native).map_err(|error| {
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
        SpecialDir::Temp => std::env::temp_dir(),
        SpecialDir::Current => std::env::current_dir().map_err(|error| {
            Error::new(
                Errc::IoError,
                format!("Platform::special_dir: current_dir failed: {error}"),
            )
        })?,
        SpecialDir::Executable => {
            let executable = std::env::current_exe().map_err(|error| {
                Error::new(
                    Errc::IoError,
                    format!("Platform::special_dir: current_exe failed: {error}"),
                )
            })?;
            executable.parent().map(Path::to_path_buf).ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "Platform::special_dir: current executable has no parent",
                )
            })?
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

// 把公开多选契约适配到 Linux 桌面文件对话框组件。
pub(crate) fn open_files(
    // 标题已经由 Platform 门面验证。
    title: &str,
    // 过滤器值已经在构造时规范化。
    filters: &[FileDialogFilter],
) -> Result<Option<Box<[PathBuf]>>> {
    // 按 Zenity 的名称竖线协议编码过滤器组。
    let zenity_filters = crate::platform::file_dialog::zenity_filters(filters);
    // 按 KDialog 的 Qt name filter 协议编码过滤器组。
    let kdialog_filters = crate::platform::file_dialog::kdialog_filters(filters);
    // 调用 crate 内部 Linux 组件并传播 typed failure。
    crate::platform::composition_root::choose_native_files(title, &zenity_filters, &kdialog_filters)
        // 把进程输出路径复制为平台公开的 owned PathBuf。
        .map(|paths| {
            // 取消保持成功空值。
            paths.map(|paths| {
                // 多选结果收窄为不可增删的 owned slice。
                paths
                    // 按 Provider 返回顺序转换每条路径。
                    .into_iter()
                    // PathBuf 不暴露子进程输出缓冲区生命周期。
                    .map(PathBuf::from)
                    // 先收集为可增长列表。
                    .collect::<Vec<_>>()
                    // 再固定为公开 owned slice。
                    .into_boxed_slice()
            })
        })
}

// 把公开保存契约适配到 Linux 桌面文件对话框组件。
pub(crate) fn save_file(
    // 标题已经由 Platform 门面验证。
    title: &str,
    // 过滤器值已经在构造时规范化。
    filters: &[FileDialogFilter],
) -> Result<Option<PathBuf>> {
    // 按 Zenity 的名称竖线协议编码过滤器组。
    let zenity_filters = crate::platform::file_dialog::zenity_filters(filters);
    // 按 KDialog 的 Qt name filter 协议编码过滤器组。
    let kdialog_filters = crate::platform::file_dialog::kdialog_filters(filters);
    // 调用 crate 内部 Linux 组件并转换 owned 路径。
    crate::platform::composition_root::choose_native_save_file(
        title,
        &zenity_filters,
        &kdialog_filters,
    )
    // 取消保持空值，确认结果转换为 owned PathBuf。
    .map(|path| path.map(PathBuf::from))
}

// 把公开目录选择契约适配到 Linux 桌面文件对话框组件。
pub(crate) fn open_folder(title: &str) -> Result<Option<PathBuf>> {
    // 调用 crate 内部 Linux 组件并转换 owned 路径。
    crate::platform::composition_root::choose_native_folder(title)
        // 取消保持空值，确认结果转换为 owned PathBuf。
        .map(|path| path.map(PathBuf::from))
}

// Linux provider 不需要 Windows 应用身份，能力查询只读探测命令可发现性。
pub(crate) fn system_notification_capability(
    // 跨平台门面统一传入身份，Linux 明确忽略。
    _app_user_model_id: Option<&AppUserModelId>,
) -> Result<SystemNotificationCapability> {
    // 仅在 PATH 中存在可执行 notify-send 时报告 Provider 已就绪。
    let capability = if super::unix_provider::command_is_available("notify-send") {
        // 可执行 Provider 已发现，调用方可以尝试发送。
        SystemNotificationCapability::Available
    } else {
        // Provider 不可发现时显式报告不支持，禁止伪装成功。
        SystemNotificationCapability::Unsupported
    };
    // 能力查询本身成功，枚举值表达当前环境状态。
    Ok(capability)
}

pub(crate) fn show_notification(
    // Linux 通知不消费 Windows AUMID。
    _app_user_model_id: Option<&AppUserModelId>,
    // 通知内容继续交给 notify-send。
    notification: &SystemNotification,
) -> Result<()> {
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
