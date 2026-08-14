use std::ffi::{OsString, c_void};
use std::os::windows::ffi::OsStringExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use crate::core::{Errc, Error, Rect, Result};
use crate::platform::hardware::{DisplayInfo, MemoryInfo, OsInfo};
use crate::platform::services::SpecialDir;
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

const COINIT_APARTMENTTHREADED: u32 = 0x2;
const RPC_E_CHANGED_MODE: i32 = 0x8001_0106_u32 as i32;
const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
const THREAD_QUERY_LIMITED_INFORMATION: u32 = 0x0800;
const INVALID_HANDLE_VALUE: *mut c_void = -1_isize as *mut c_void;

/// 当前 owner thread 持有的 Windows apartment 初始化配额。
pub(crate) struct State {
    com_initialized: bool,
}

impl State {
    pub(crate) fn new() -> Result<Self> {
        // SAFETY: null reserved pointer is required by COM; this call is balanced
        // by State::drop on the same thread for every successful HRESULT.
        let result = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
        match result {
            0 | 1 => Ok(Self {
                com_initialized: true,
            }),
            RPC_E_CHANGED_MODE => Err(Error::new(
                Errc::InvalidState,
                "Platform::new: owner thread already uses an incompatible COM apartment",
            )),
            value => Err(Error::new(
                Errc::PlatformError,
                format!("Platform::new: CoInitializeEx failed with HRESULT 0x{value:08X}"),
            )),
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if self.com_initialized {
            // SAFETY: Platform is !Send and State is dropped on the owner thread;
            // this balances the successful CoInitializeEx in State::new.
            unsafe { CoUninitialize() };
            self.com_initialized = false;
        }
    }
}

// Windows 主线程默认栈较小；深层 ViewNode 树的构建、协调与布局递归需要
// 有界大栈 UI 线程，与应用入口约定的容量一致。
const UI_THREAD_STACK_BYTES: usize = 8 * 1024 * 1024;

// 在命名的大栈 UI 线程上运行闭包，并把线程结果或 panic 恢复到调用方。
pub(crate) fn run_on_ui_thread<F, R>(thread_name: &str, run: F) -> R
where
    // 闭包与返回值都进入独立线程，必须可发送且不借用调用栈。
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    // 用目标名与固定大栈创建专用 UI 线程。
    let ui_thread = match std::thread::Builder::new()
        .name(thread_name.to_owned())
        .stack_size(UI_THREAD_STACK_BYTES)
        .spawn(run) {
        // 返回已创建的 UI 线程。
        Ok(ui_thread) => ui_thread,
        // 线程创建失败属于不可恢复的平台错误，文案带线程名便于诊断。
        Err(error) => panic!("spawn UI thread {thread_name:?}: {error}"),
    };
    // 等待 UI 线程结束；panic 按原样恢复到调用线程。
    match ui_thread.join() {
        // 返回闭包结果。
        Ok(result) => result,
        // 把子线程 panic 负载恢复到当前线程继续展开。
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

pub(crate) fn is_main_thread() -> Result<bool> {
    // Windows exposes no direct main-thread predicate. The initial process
    // thread is the live process thread with the earliest creation timestamp.
    // SAFETY: all snapshot and thread handles are closed on every exit path.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
            return Err(last_error("CreateToolhelp32Snapshot"));
        }
        let snapshot_guard = HandleGuard(snapshot);
        let process_id = GetCurrentProcessId();
        let current_thread_id = GetCurrentThreadId();
        let mut entry = ThreadEntry32 {
            size: std::mem::size_of::<ThreadEntry32>() as u32,
            ..ThreadEntry32::default()
        };
        if Thread32First(snapshot_guard.0, &mut entry) == 0 {
            return Err(last_error("Thread32First"));
        }

        let mut earliest: Option<(u64, u32)> = None;
        loop {
            if entry.owner_process_id == process_id {
                let handle = OpenThread(THREAD_QUERY_LIMITED_INFORMATION, 0, entry.thread_id);
                if !handle.is_null() {
                    let handle = HandleGuard(handle);
                    let mut creation = FileTime::default();
                    let mut exit = FileTime::default();
                    let mut kernel = FileTime::default();
                    let mut user = FileTime::default();
                    if GetThreadTimes(handle.0, &mut creation, &mut exit, &mut kernel, &mut user)
                        != 0
                    {
                        let timestamp = creation.as_u64();
                        if earliest.as_ref().is_none_or(|&(old, id)| {
                            timestamp < old || (timestamp == old && entry.thread_id < id)
                        }) {
                            earliest = Some((timestamp, entry.thread_id));
                        }
                    }
                }
            }

            entry.size = std::mem::size_of::<ThreadEntry32>() as u32;
            if Thread32Next(snapshot_guard.0, &mut entry) == 0 {
                let source = std::io::Error::last_os_error();
                if source.raw_os_error() != Some(18) {
                    return Err(Error::new(
                        Errc::PlatformError,
                        format!("Platform::new: Thread32Next failed: {source}"),
                    ));
                }
                break;
            }
        }

        earliest
            .map(|(_, thread_id)| thread_id == current_thread_id)
            .ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "Platform::new: could not identify the process main thread",
                )
            })
    }
}

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

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _device_context: HDC,
    _bounds: *mut RECT,
    data: LPARAM,
) -> BOOL {
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

struct HandleGuard(*mut c_void);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: HandleGuard uniquely owns a closeable Win32 handle.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
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
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}

impl FileTime {
    fn as_u64(&self) -> u64 {
        (u64::from(self.high) << 32) | u64::from(self.low)
    }
}

#[repr(C)]
#[derive(Default)]
struct ThreadEntry32 {
    size: u32,
    usage: u32,
    thread_id: u32,
    owner_process_id: u32,
    base_priority: i32,
    delta_priority: i32,
    flags: u32,
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
unsafe extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
    fn CoUninitialize();
    fn CoTaskMemFree(memory: *mut c_void);
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(
        folder: *const Guid,
        flags: u32,
        token: *mut c_void,
        path: *mut *mut u16,
    ) -> i32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn RtlGetVersion(version: *mut RtlOsVersionInfo) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
    fn GetCurrentProcessId() -> u32;
    fn GetCurrentThreadId() -> u32;
    fn GetThreadTimes(
        thread: *mut c_void,
        creation: *mut FileTime,
        exit: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn GlobalMemoryStatusEx(memory: *mut MemoryStatusEx) -> i32;
    fn OpenThread(access: u32, inherit_handle: i32, thread_id: u32) -> *mut c_void;
    fn Thread32First(snapshot: *mut c_void, entry: *mut ThreadEntry32) -> i32;
    fn Thread32Next(snapshot: *mut c_void, entry: *mut ThreadEntry32) -> i32;
}
