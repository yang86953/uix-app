use std::ffi::{CStr, c_char, c_void};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::core::{Errc, Error, Result};
use crate::native::backends::macos::platform::MacosPlatform;
use crate::native::platform::Platform as NativePlatform;
use crate::platform::hardware::{DisplayInfo, MemoryInfo, OsInfo};
// 引入跨平台通知身份与能力状态契约。
use crate::platform::services::{
    AppUserModelId, SpecialDir, SystemNotification, SystemNotificationCapability,
};

pub(crate) struct State;

impl State {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self)
    }
}

pub(crate) fn is_main_thread() -> Result<bool> {
    // SAFETY: pthread_main_np has no arguments or ownership side effects.
    Ok(unsafe { pthread_main_np() } != 0)
}

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
    let platform = MacosPlatform::new(crate::diagnostics::PendingFailureQueue::new());
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
    };
    Ok(path)
}

// macOS provider 不需要 Windows 应用身份，命令故障由发送结果报告。
pub(crate) fn system_notification_capability(
    // 跨平台门面统一传入身份，macOS 明确忽略。
    _app_user_model_id: Option<&AppUserModelId>,
) -> Result<SystemNotificationCapability> {
    // osascript provider 已编译进入当前目标。
    Ok(SystemNotificationCapability::Available)
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
extern "C" {
    fn pthread_main_np() -> i32;
    fn sysctlbyname(
        name: *const c_char,
        old_value: *mut c_void,
        old_length: *mut usize,
        new_value: *mut c_void,
        new_length: usize,
    ) -> i32;
}
