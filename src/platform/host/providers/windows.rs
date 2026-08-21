use std::ffi::{OsString, c_void};
use std::os::windows::ffi::OsStringExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use crate::core::{Errc, Error, Rect, Result};
use crate::platform::hardware::{DisplayInfo, MemoryInfo, OsInfo};
// 引入文件对话框公开过滤器值与系统目录契约。
use crate::platform::services::{FileDialogFilter, SpecialDir};
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW,
    HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::core::{BOOL, PCWSTR};

// Windows 通知 Adapter 独立成组件，避免平台硬件查询文件超过行数门禁。
mod notification;
// 向上层门面重导出通知能力查询与发送入口。
pub(crate) use notification::{show_notification, system_notification_capability};

pub(crate) fn os_info() -> Result<OsInfo> {
    // SAFETY: RtlGetVersion fills the correctly-sized POD structure.
    unsafe {
        let mut version = RtlOsVersionInfo {
            size: std::mem::size_of::<RtlOsVersionInfo>() as u32,
            ..RtlOsVersionInfo::default()
        };
        let status = RtlGetVersion(&mut version);
        if status != 0 {
            return Err(Error::new(
                Errc::PlatformError,
                format!("Platform::os_info: RtlGetVersion failed with NTSTATUS 0x{status:08X}"),
            ));
        }
        let name = if version.major == 10 && version.minor == 0 {
            if version.build >= 22_000 {
                "Windows 11"
            } else {
                "Windows 10"
            }
        } else {
            "Windows"
        };
        Ok(OsInfo::new(
            name.to_owned(),
            Some(format!("{}.{}", version.major, version.minor)),
            Some(version.build.to_string()),
        ))
    }
}

pub(crate) fn cpu_metadata() -> (Option<String>, Option<String>) {
    let model = std::env::var("PROCESSOR_IDENTIFIER")
        .ok()
        .and_then(non_empty);
    let vendor = model.as_deref().and_then(|identifier| {
        ["GenuineIntel", "AuthenticAMD", "ARM", "Qualcomm"]
            .into_iter()
            .find(|candidate| identifier.contains(candidate))
            .map(str::to_owned)
    });
    (vendor, model)
}

pub(crate) fn memory_info() -> Result<MemoryInfo> {
    let mut memory = MemoryStatusEx {
        length: std::mem::size_of::<MemoryStatusEx>() as u32,
        ..MemoryStatusEx::default()
    };
    // SAFETY: memory has the documented size and remains writable for the call.
    if unsafe { GlobalMemoryStatusEx(&mut memory) } == 0 {
        return Err(last_error("GlobalMemoryStatusEx"));
    }
    if memory.total_physical == 0 || memory.available_physical > memory.total_physical {
        return Err(Error::new(
            Errc::PlatformError,
            "Platform::memory_info: Windows returned invalid physical-memory totals",
        ));
    }
    Ok(MemoryInfo::new(
        memory.total_physical,
        memory.available_physical,
    ))
}

pub(crate) fn displays() -> Result<Box<[DisplayInfo]>> {
    let mut inventory = MonitorInventory::default();
    let pointer = (&mut inventory as *mut MonitorInventory) as isize;
    // SAFETY: EnumDisplayMonitors invokes collect_monitor synchronously; pointer
    // remains valid and uniquely borrowed for the complete enumeration.
    let completed =
        unsafe { EnumDisplayMonitors(None, None, Some(collect_monitor), LPARAM(pointer)) };
    if inventory.panicked {
        return Err(Error::new(
            Errc::PlatformError,
            "Platform::displays: EnumDisplayMonitors callback panicked",
        ));
    }
    if !completed.as_bool() {
        return Err(Error::new(
            Errc::PlatformError,
            inventory
                .failure
                .unwrap_or_else(|| "Platform::displays: EnumDisplayMonitors failed".to_owned()),
        ));
    }
    if let Some(failure) = inventory.failure {
        return Err(Error::new(Errc::PlatformError, failure));
    }
    inventory.values.sort_by_key(|display| {
        let bounds = display.bounds();
        (!display.is_primary(), bounds.y as i64, bounds.x as i64)
    });
    Ok(inventory.values.into_boxed_slice())
}

pub(crate) fn special_dir(directory: SpecialDir) -> Result<PathBuf> {
    let folder = match directory {
        SpecialDir::Home => FOLDERID_PROFILE,
        SpecialDir::AppData => FOLDERID_ROAMING_APP_DATA,
        SpecialDir::LocalAppData => FOLDERID_LOCAL_APP_DATA,
        SpecialDir::Documents => FOLDERID_DOCUMENTS,
        SpecialDir::Desktop => FOLDERID_DESKTOP,
        SpecialDir::Downloads => FOLDERID_DOWNLOADS,
        SpecialDir::Temp => return Ok(std::env::temp_dir()),
        SpecialDir::Current => {
            return std::env::current_dir().map_err(|error| {
                Error::new(
                    Errc::IoError,
                    format!("Platform::special_dir: current_dir failed: {error}"),
                )
            });
        }
        SpecialDir::Executable => {
            let executable = std::env::current_exe().map_err(|error| {
                Error::new(
                    Errc::IoError,
                    format!("Platform::special_dir: current_exe failed: {error}"),
                )
            })?;
            return executable.parent().map(PathBuf::from).ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "Platform::special_dir: current executable has no parent",
                )
            });
        }
    };
    // SAFETY: folder points at a valid GUID; Shell allocates path and ownership
    // is released exactly once with CoTaskMemFree.
    unsafe {
        let mut pointer: *mut u16 = std::ptr::null_mut();
        let result = SHGetKnownFolderPath(&folder, 0, std::ptr::null_mut(), &mut pointer);
        let path_memory = CoTaskMemPath(pointer);
        if result < 0 {
            let code = if result == 0x8007_0005_u32 as i32 {
                Errc::PermissionDenied
            } else {
                Errc::PlatformError
            };
            return Err(Error::new(
                code,
                format!(
                    "Platform::special_dir: SHGetKnownFolderPath failed with HRESULT 0x{result:08X}"
                ),
            ));
        }
        if path_memory.0.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "Platform::special_dir: SHGetKnownFolderPath returned a null path",
            ));
        }
        let mut length = 0usize;
        while length < 32_768 && *path_memory.0.add(length) != 0 {
            length += 1;
        }
        if length == 32_768 {
            return Err(Error::new(
                Errc::PlatformError,
                "Platform::special_dir: known-folder path exceeds the Win32 limit",
            ));
        }
        let path = PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(
            path_memory.0,
            length,
        )));
        if path.as_os_str().is_empty() {
            Err(Error::new(
                Errc::NotFound,
                "Platform::special_dir: Windows returned an empty known-folder path",
            ))
        } else {
            Ok(path)
        }
    }
}

// 把公开多选契约适配到 Win32 common dialog 组件。
pub(crate) fn open_files(
    // 标题已经由 Platform 门面验证。
    title: &str,
    // 过滤器值已经在构造时规范化。
    filters: &[FileDialogFilter],
) -> Result<Option<Box<[PathBuf]>>> {
    // 编码 OPENFILENAMEW 要求的双 NUL 过滤器列表。
    let filters = crate::platform::file_dialog::windows_filters(filters);
    // 调用 crate 内部 Win32 组件并传播 typed failure。
    crate::platform::composition_root::choose_native_files(title, &filters)
        // 把 UTF-8 内部路径复制为平台公开的 owned PathBuf。
        .map(|paths| {
            // 取消保持成功空值。
            paths.map(|paths| {
                // 多选结果收窄为不可增删的 owned slice。
                paths
                    // 按原生返回顺序转换每条路径。
                    .into_iter()
                    // PathBuf 不暴露 Win32 缓冲区生命周期。
                    .map(PathBuf::from)
                    // 先收集为可增长列表。
                    .collect::<Vec<_>>()
                    // 再固定为公开 owned slice。
                    .into_boxed_slice()
            })
        })
}

// 把公开保存契约适配到 Win32 common dialog 组件。
pub(crate) fn save_file(
    // 标题已经由 Platform 门面验证。
    title: &str,
    // 过滤器值已经在构造时规范化。
    filters: &[FileDialogFilter],
) -> Result<Option<PathBuf>> {
    // 编码 OPENFILENAMEW 要求的双 NUL 过滤器列表。
    let filters = crate::platform::file_dialog::windows_filters(filters);
    // 调用 crate 内部 Win32 组件并传播 typed failure。
    crate::platform::composition_root::choose_native_save_file(title, &filters)
        // 取消保持空值，确认结果转换为 owned PathBuf。
        .map(|path| path.map(PathBuf::from))
}

// 把公开目录选择契约适配到 Win32 shell 组件。
pub(crate) fn open_folder(title: &str) -> Result<Option<PathBuf>> {
    // 调用 crate 内部 Win32 组件并转换 owned 路径。
    crate::platform::composition_root::choose_native_folder(title)
        // 取消保持空值，确认结果转换为 owned PathBuf。
        .map(|path| path.map(PathBuf::from))
}

// SAFETY: 该回调只由 displays() 的同步枚举调用，data 必须指向枚举期间唯一存活的 MonitorInventory。
unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _device_context: HDC,
    _bounds: *mut RECT,
    data: LPARAM,
) -> BOOL {
    // SAFETY: EnumDisplayMonitors 传回原始 monitor 和 data；外层捕获 panic，避免异常跨越 FFI 边界。
    match catch_unwind(AssertUnwindSafe(|| unsafe {
        collect_monitor_unchecked(monitor, data)
    })) {
        Ok(result) => result,
        Err(_) => {
            if data.0 != 0 {
                // SAFETY: the synchronous EnumDisplayMonitors caller keeps this
                // inventory alive until the callback returns.
                unsafe { (*(data.0 as *mut MonitorInventory)).panicked = true };
            }
            BOOL(0)
        }
    }
}

unsafe fn collect_monitor_unchecked(monitor: HMONITOR, data: LPARAM) -> BOOL {
    if data.0 == 0 {
        return BOOL(0);
    }
    // SAFETY: data is the unique MonitorInventory passed by displays().
    let inventory = unsafe { &mut *(data.0 as *mut MonitorInventory) };
    let mut monitor_info = MONITORINFOEXW::default();
    monitor_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    // SAFETY: MONITORINFOEXW begins with MONITORINFO per the Win32 ABI.
    if !unsafe {
        GetMonitorInfoW(
            monitor,
            (&mut monitor_info as *mut MONITORINFOEXW).cast::<MONITORINFO>(),
        )
    }
    .as_bool()
    {
        inventory.failure = Some("Platform::displays: GetMonitorInfoW failed".to_owned());
        return BOOL(0);
    }

    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    // SAFETY: monitor is supplied by EnumDisplayMonitors and outputs are valid.
    if let Err(error) =
        unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }
    {
        inventory.failure = Some(format!(
            "Platform::displays: GetDpiForMonitor failed: {error}"
        ));
        return BOOL(0);
    }
    let scale = dpi_x as f32 / 96.0;
    if !scale.is_finite() || scale <= 0.0 {
        inventory.failure = Some(format!(
            "Platform::displays: Windows returned invalid DPI {dpi_x}"
        ));
        return BOOL(0);
    }

    let physical = monitor_info.monitorInfo.rcMonitor;
    let width = physical.right.saturating_sub(physical.left);
    let height = physical.bottom.saturating_sub(physical.top);
    if width <= 0 || height <= 0 {
        inventory.failure =
            Some("Platform::displays: Windows returned invalid monitor bounds".to_owned());
        return BOOL(0);
    }
    let name = wide_text(&monitor_info.szDevice);
    let refresh_rate_millihertz = name.as_ref().and_then(|_| {
        let mut mode = DEVMODEW {
            dmSize: std::mem::size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        // SAFETY: szDevice is NUL-terminated by GetMonitorInfoW and mode has the
        // documented size.
        unsafe {
            EnumDisplaySettingsW(
                PCWSTR(monitor_info.szDevice.as_ptr()),
                ENUM_CURRENT_SETTINGS,
                &mut mode,
            )
        }
        .as_bool()
        .then_some(mode.dmDisplayFrequency)
        .filter(|frequency| *frequency > 1)
        .and_then(|frequency| frequency.checked_mul(1_000))
    });

    inventory.values.push(DisplayInfo::new(
        Rect::new(
            physical.left as f32 / scale,
            physical.top as f32 / scale,
            width as f32 / scale,
            height as f32 / scale,
        ),
        scale,
        monitor_info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        name,
        refresh_rate_millihertz,
    ));
    BOOL(1)
}

#[derive(Default)]
struct MonitorInventory {
    values: Vec<DisplayInfo>,
    failure: Option<String>,
    panicked: bool,
}

fn wide_text(value: &[u16]) -> Option<String> {
    let length = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    non_empty(String::from_utf16_lossy(&value[..length]))
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn last_error(operation: &str) -> Error {
    let source = std::io::Error::last_os_error();
    let code = match source.kind() {
        std::io::ErrorKind::PermissionDenied => Errc::PermissionDenied,
        _ => Errc::PlatformError,
    };
    Error::new(code, format!("Platform: {operation} failed: {source}"))
}

struct CoTaskMemPath(*mut u16);

impl Drop for CoTaskMemPath {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: SHGetKnownFolderPath transfers this allocation to the
            // caller and requires exactly one CoTaskMemFree.
            unsafe { CoTaskMemFree(self.0.cast()) };
        }
    }
}

#[repr(C)]
struct RtlOsVersionInfo {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform_id: u32,
    service_pack: [u16; 128],
}

impl Default for RtlOsVersionInfo {
    fn default() -> Self {
        Self {
            size: 0,
            major: 0,
            minor: 0,
            build: 0,
            platform_id: 0,
            service_pack: [0; 128],
        }
    }
}

#[repr(C)]
#[derive(Default)]
struct MemoryStatusEx {
    length: u32,
    memory_load: u32,
    total_physical: u64,
    available_physical: u64,
    total_page_file: u64,
    available_page_file: u64,
    total_virtual: u64,
    available_virtual: u64,
    available_extended_virtual: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const FOLDERID_PROFILE: Guid = Guid {
    data1: 0x5E6C858F,
    data2: 0x0E22,
    data3: 0x4760,
    data4: [0x9A, 0xFE, 0xEA, 0x33, 0x17, 0xB6, 0x73, 0x73],
};
const FOLDERID_ROAMING_APP_DATA: Guid = Guid {
    data1: 0x3EB685DB,
    data2: 0x65F9,
    data3: 0x4CF6,
    data4: [0xA0, 0x3A, 0xE3, 0xEF, 0x65, 0x72, 0x9F, 0x3D],
};
const FOLDERID_LOCAL_APP_DATA: Guid = Guid {
    data1: 0xF1B32785,
    data2: 0x6FBA,
    data3: 0x4FCF,
    data4: [0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91],
};
const FOLDERID_DOCUMENTS: Guid = Guid {
    data1: 0xFDD39AD0,
    data2: 0x238F,
    data3: 0x46AF,
    data4: [0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7],
};
const FOLDERID_DESKTOP: Guid = Guid {
    data1: 0xB4BFCC3A,
    data2: 0xDB2C,
    data3: 0x424C,
    data4: [0xB0, 0x29, 0x7F, 0xE9, 0x9A, 0x87, 0xC6, 0x41],
};
const FOLDERID_DOWNLOADS: Guid = Guid {
    data1: 0x374DE290,
    data2: 0x123F,
    data3: 0x4565,
    data4: [0x91, 0x64, 0x39, 0xC4, 0x92, 0x5E, 0x46, 0x7B],
};

#[link(name = "ole32")]
// SAFETY: 声明与 ole32 的 Win32 ABI 一致，调用方按线程配对 COM 初始化并只释放 COM 分配的内存。
unsafe extern "system" {
    fn CoTaskMemFree(memory: *mut c_void);
}

#[link(name = "shell32")]
// SAFETY: 声明与 shell32 的 Win32 ABI 一致，调用方提供有效 GUID 和可写输出槽并接管返回内存。
unsafe extern "system" {
    fn SHGetKnownFolderPath(
        folder: *const Guid,
        flags: u32,
        token: *mut c_void,
        path: *mut *mut u16,
    ) -> i32;
}

#[link(name = "ntdll")]
// SAFETY: RtlGetVersion 声明与 ntdll ABI 一致，调用方提供已填写结构尺寸的可写对象。
unsafe extern "system" {
    fn RtlGetVersion(version: *mut RtlOsVersionInfo) -> i32;
}

#[link(name = "kernel32")]
// SAFETY: 本块声明与 kernel32 ABI 一致，调用方负责所有句柄、结构尺寸和输出指针的有效性。
unsafe extern "system" {
    fn GlobalMemoryStatusEx(memory: *mut MemoryStatusEx) -> i32;
}
