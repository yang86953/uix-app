//! Windows 的 Platform 实例生命周期与主线程识别实现。

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};

// Platform owner thread 使用单线程 COM apartment。
const COINIT_APARTMENTTHREADED: u32 = 0x2;
// 已存在不兼容 apartment 时不能接管 owner thread。
const RPC_E_CHANGED_MODE: i32 = 0x8001_0106_u32 as i32;
// 线程快照标志。
const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
// 查询线程创建时间所需的最小权限。
const THREAD_QUERY_LIMITED_INFORMATION: u32 = 0x0800;
// Win32 无效句柄哨兵。
const INVALID_HANDLE_VALUE: *mut c_void = -1_isize as *mut c_void;

/// 当前 owner thread 持有的 Windows apartment 初始化配额。
pub(crate) struct State {
    // 记录是否需要在析构时配对调用 CoUninitialize。
    com_initialized: bool,
}

impl State {
    // 初始化 owner thread 的 COM apartment。
    pub(crate) fn new() -> Result<Self> {
        // SAFETY: COM 要求 reserved 为空；成功结果由同线程析构配对释放。
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
    // 在 owner thread 上释放 COM apartment 配额。
    fn drop(&mut self) {
        if self.com_initialized {
            // SAFETY: Platform 不可跨线程移动，调用与成功的 CoInitializeEx 严格配对。
            unsafe { CoUninitialize() };
            self.com_initialized = false;
        }
    }
}

// Windows 以进程内最早创建的存活线程作为主线程。
pub(crate) fn is_main_thread() -> Result<bool> {
    // SAFETY: 本块持有并在所有退出路径关闭快照与线程句柄。
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

// 把最后一个 Win32 错误映射为 UIX 平台错误。
fn last_error(operation: &str) -> Error {
    let source = std::io::Error::last_os_error();
    let code = match source.kind() {
        std::io::ErrorKind::PermissionDenied => Errc::PermissionDenied,
        _ => Errc::PlatformError,
    };
    Error::new(code, format!("Platform: {operation} failed: {source}"))
}

// 自动关闭 Win32 快照或线程句柄。
struct HandleGuard(*mut c_void);

impl Drop for HandleGuard {
    // 仅关闭有效且由本对象独占的句柄。
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: HandleGuard 独占一个可关闭的 Win32 句柄。
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

// Win32 FILETIME 的本地 ABI 映射。
#[repr(C)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}

impl FileTime {
    // 合并高低位为可排序时间戳。
    fn as_u64(&self) -> u64 {
        (u64::from(self.high) << 32) | u64::from(self.low)
    }
}

// Toolhelp 线程条目的本地 ABI 映射。
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

#[link(name = "ole32")]
// SAFETY: 声明与 ole32 的 Win32 ABI 一致，并由 State 在同线程配对调用。
unsafe extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
    fn CoUninitialize();
}

#[link(name = "kernel32")]
// SAFETY: 声明与 kernel32 ABI 一致，调用方维护结构尺寸、输出指针与句柄生命周期。
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
    fn OpenThread(access: u32, inherit_handle: i32, thread_id: u32) -> *mut c_void;
    fn Thread32First(snapshot: *mut c_void, entry: *mut ThreadEntry32) -> i32;
    fn Thread32Next(snapshot: *mut c_void, entry: *mut ThreadEntry32) -> i32;
}
