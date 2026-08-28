use std::ffi::{CStr, c_char, c_void};
use std::path::PathBuf;
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
    let version = sysctl_string(c"kern.osproductversion").ok();
    let build = sysctl_string(c"kern.osversion").ok();
    Ok(OsInfo::new(
        "macOS".to_owned(),
        version.and_then(non_empty),
        build.and_then(non_empty),
    ))
}

pub(crate) fn cpu_metadata() -> (Option<String>, Option<String>) {
    let vendor = sysctl_string(c"machdep.cpu.vendor")
        .ok()
        .and_then(non_empty);
    let model = sysctl_string(c"machdep.cpu.brand_string")
        .or_else(|_| sysctl_string(c"hw.model"))
        .ok()
        .and_then(non_empty);
    (vendor, model)
}

pub(crate) fn memory_info() -> Result<MemoryInfo> {
    let total = sysctl_integer(c"hw.memsize")?;
    let page_size = sysctl_integer(c"hw.pagesize")?;
    let free = sysctl_integer(c"vm.page_free_count")?;
    let inactive = sysctl_integer(c"vm.page_inactive_count").unwrap_or(0);
    let speculative = sysctl_integer(c"vm.page_speculative_count").unwrap_or(0);
    let pages = free
        .checked_add(inactive)
        .and_then(|value| value.checked_add(speculative))
        .ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "Platform::memory_info: macOS available-page count overflowed",
            )
        })?;
    let available = pages.checked_mul(page_size).ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            "Platform::memory_info: macOS available-memory byte count overflowed",
        )
    })?;
    if total == 0 || available > total {
        return Err(Error::new(
            Errc::PlatformError,
            "Platform::memory_info: macOS returned invalid physical-memory totals",
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
            "Platform::displays: AppKit returned a negative display count",
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
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            Error::new(
                Errc::NotFound,
                "Platform::special_dir: HOME is not available",
            )
        })?;
    let path = match directory {
        SpecialDir::Home => home,
        SpecialDir::AppData | SpecialDir::LocalAppData => {
            home.join("Library").join("Application Support")
        }
        SpecialDir::Documents => home.join("Documents"),
        SpecialDir::Desktop => home.join("Desktop"),
        SpecialDir::Downloads => home.join("Downloads"),
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
            executable.parent().map(PathBuf::from).ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "Platform::special_dir: current executable has no parent",
                )
            })?
        }
    };
    Ok(path)
}

// 把公开多选契约适配到 AppKit 文件面板组件。
pub(crate) fn open_files(
    // 标题已经由 Platform 门面验证。
    title: &str,
    // 过滤器值已经在构造时规范化。
    filters: &[FileDialogFilter],
) -> Result<Option<Box<[PathBuf]>>> {
    // 只编码扩展名模式，避免 AppKit 把小写显示名称误识别为文件类型。
    let filters = crate::platform::file_dialog::macos_filters(filters);
    // 调用 crate 内部 AppKit 组件并传播 typed failure。
    crate::platform::composition_root::choose_native_files(title, &filters)
        // 把 AppKit UTF-8 路径复制为平台公开的 owned PathBuf。
        .map(|paths| {
            // 取消保持成功空值。
            paths.map(|paths| {
                // 多选结果收窄为不可增删的 owned slice。
                paths
                    // 按 AppKit 返回顺序转换每条路径。
                    .into_iter()
                    // PathBuf 不暴露 Objective-C 对象生命周期。
                    .map(PathBuf::from)
                    // 先收集为可增长列表。
                    .collect::<Vec<_>>()
                    // 再固定为公开 owned slice。
                    .into_boxed_slice()
            })
        })
}

// 把公开保存契约适配到 AppKit 文件面板组件。
pub(crate) fn save_file(
    // 标题已经由 Platform 门面验证。
    title: &str,
    // 过滤器值已经在构造时规范化。
    filters: &[FileDialogFilter],
) -> Result<Option<PathBuf>> {
    // 只编码扩展名模式，避免 AppKit 把小写显示名称误识别为文件类型。
    let filters = crate::platform::file_dialog::macos_filters(filters);
    // 调用 crate 内部 AppKit 组件并转换 owned 路径。
    crate::platform::composition_root::choose_native_save_file(title, &filters)
        // 取消保持空值，确认结果转换为 owned PathBuf。
        .map(|path| path.map(PathBuf::from))
}

// 把公开目录选择契约适配到 AppKit 文件面板组件。
pub(crate) fn open_folder(title: &str) -> Result<Option<PathBuf>> {
    // 调用 crate 内部 AppKit 组件并转换 owned 路径。
    crate::platform::composition_root::choose_native_folder(title)
        // 取消保持空值，确认结果转换为 owned PathBuf。
        .map(|path| path.map(PathBuf::from))
}

// macOS provider 不需要 Windows 应用身份，能力查询只读探测命令可发现性。
pub(crate) fn system_notification_capability(
    // 跨平台门面统一传入身份，macOS 明确忽略。
    _app_user_model_id: Option<&AppUserModelId>,
) -> Result<SystemNotificationCapability> {
    // 仅在 PATH 中存在可执行 osascript 时报告 Provider 已就绪。
    let capability = if super::unix_provider::command_is_available("osascript") {
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
    // macOS 通知不消费 Windows AUMID。
    _app_user_model_id: Option<&AppUserModelId>,
    // 通知内容继续交给 osascript。
    notification: &SystemNotification,
) -> Result<()> {
    let status = Command::new("osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display notification item 2 of argv with title item 1 of argv",
            "-e",
            "end run",
            "--",
        ])
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
                format!("Platform::show_notification: cannot launch osascript: {source}"),
            )
        })?;
    if status.success() {
        return Ok(());
    }
    Err(Error::new(
        Errc::PlatformError,
        format!("Platform::show_notification: osascript exited with {status}"),
    ))
}

fn sysctl_string(name: &CStr) -> Result<String> {
    let bytes = sysctl_bytes(name)?;
    let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes.as_slice());
    String::from_utf8(bytes.to_vec()).map_err(|source| {
        Error::new(
            Errc::PlatformError,
            format!(
                "Platform hardware query: {} is not valid UTF-8: {source}",
                name.to_string_lossy()
            ),
        )
    })
}

fn sysctl_integer(name: &CStr) -> Result<u64> {
    let bytes = sysctl_bytes(name)?;
    match bytes.as_slice() {
        [a, b, c, d] => Ok(u32::from_ne_bytes([*a, *b, *c, *d]).into()),
        [a, b, c, d, e, f, g, h] => Ok(u64::from_ne_bytes([*a, *b, *c, *d, *e, *f, *g, *h])),
        _ => Err(Error::new(
            Errc::PlatformError,
            format!(
                "Platform hardware query: {} returned an unsupported integer size {}",
                name.to_string_lossy(),
                bytes.len()
            ),
        )),
    }
}

fn sysctl_bytes(name: &CStr) -> Result<Vec<u8>> {
    let mut length = 0usize;
    // SAFETY: first call requests required length and writes no value bytes.
    if unsafe {
        sysctlbyname(
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(sysctl_error(name));
    }
    if length == 0 || length > 1024 * 1024 {
        return Err(Error::new(
            Errc::PlatformError,
            format!(
                "Platform hardware query: {} returned invalid size {length}",
                name.to_string_lossy()
            ),
        ));
    }
    let mut value = vec![0u8; length];
    // SAFETY: value has exactly the capacity reported by the first call.
    if unsafe {
        sysctlbyname(
            name.as_ptr(),
            value.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(sysctl_error(name));
    }
    value.truncate(length);
    Ok(value)
}

fn sysctl_error(name: &CStr) -> Error {
    let source = std::io::Error::last_os_error();
    let code = match source.kind() {
        std::io::ErrorKind::NotFound => Errc::NotImplemented,
        std::io::ErrorKind::PermissionDenied => Errc::PermissionDenied,
        _ => Errc::PlatformError,
    };
    Error::new(
        code,
        format!(
            "Platform hardware query: sysctl {} failed: {source}",
            name.to_string_lossy()
        ),
    )
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

#[link(name = "System")]
// Rust 2024 要求显式标记外部符号声明块的调用安全边界。
unsafe extern "C" {
    fn sysctlbyname(
        name: *const c_char,
        old_value: *mut c_void,
        old_length: *mut usize,
        new_value: *mut c_void,
        new_length: usize,
    ) -> i32;
}
