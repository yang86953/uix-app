// ============================================================================
// native/backends/windows/system_info.rs — Windows system info impl (ISystemInfo)
// ============================================================================

#![cfg(windows)]
#![allow(nonstandard_style)]
#![allow(clippy::upper_case_acronyms)]

use crate::native::backends::windows::util::to_utf8;
use crate::native::capabilities::system::ISystemInfo;
use crate::native::capabilities::system::{MemoryInfo, OsInfo};
use crate::native::{Errc, Error, Result};

// ════════════════════════════════════════════════════════════════════════════
// WindowsSystemInfo
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
// Windows 系统信息后端只在 crate 内部平台注册表中构造。
pub(crate) struct WindowsSystemInfo;

impl WindowsSystemInfo {
    // 创建无状态 Windows 系统信息后端。
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for WindowsSystemInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ISystemInfo for WindowsSystemInfo {
    fn os_info(&self) -> Result<OsInfo> {
        get_os_info()
    }

    fn cpu_count(&self) -> Result<u32> {
        get_cpu_count()
    }

    fn memory_info(&self) -> Result<MemoryInfo> {
        get_memory_info()
    }

    fn hostname(&self) -> Result<String> {
        get_hostname()
    }

    fn username(&self) -> Result<String> {
        get_username()
    }

    fn up_time(&self) -> Result<u64> {
        get_uptime_ms()
    }

    fn default_font_paths(&self) -> Result<Vec<String>> {
        Ok(crate::native::backends::windows::util::system_default_font_paths())
    }

    fn probe_cjk_font_path(&self) -> Option<String> {
        crate::native::backends::windows::util::probe_cjk_font_path()
    }

    fn probe_cjk_font_paths(&self) -> Vec<String> {
        crate::native::backends::windows::util::probe_cjk_font_paths()
    }

    fn probe_family_font_path(&self, family: &str) -> Option<String> {
        crate::native::backends::windows::util::probe_family_font_path(family)
    }

    fn scan_fallback_font_path(&self) -> Option<String> {
        crate::native::backends::windows::util::scan_random_font_path()
    }

    fn process_memory(&self) -> Result<(usize, usize)> {
        get_process_memory()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 内部实现
// ════════════════════════════════════════════════════════════════════════════

fn get_os_info() -> Result<OsInfo> {
    // Use RtlGetVersion to get accurate OS version (not affected by manifest compat)
    // SAFETY: ver 为完整初始化的版本结构（dwOSVersionInfoSize 已设置），RtlGetVersion 只写入它；system_info 为栈上默认初始化结构。
    unsafe {
        let mut ver = RTL_OSVERSIONINFOW {
            dwOSVersionInfoSize: std::mem::size_of::<RTL_OSVERSIONINFOW>() as u32,
            dwMajorVersion: 0,
            dwMinorVersion: 0,
            dwBuildNumber: 0,
            dwPlatformId: 0,
            szCSDVersion: [0u16; 128],
        };

        let status = RtlGetVersion(&mut ver);
        if status != 0 {
            return Err(system_info_error("RtlGetVersion"));
        }
        let mut is_64bit = false;
        let mut system_info = SYSTEM_INFO::default();
        GetNativeSystemInfo(&mut system_info);
        match system_info.wProcessorArchitecture {
            0 => { /* x86 */ }
            9 => {
                is_64bit = true;
            } // AMD64
            12 => {
                is_64bit = true;
            } // ARM64
            6 => {
                /* IA64 */
                is_64bit = true;
            }
            _ => {}
        }

        let version_str = format!("{}.{}", ver.dwMajorVersion, ver.dwMinorVersion);
        let build_str = format!("{}", ver.dwBuildNumber);

        // Determine OS name
        let name = match (ver.dwMajorVersion, ver.dwMinorVersion) {
            (10, 0) => {
                if ver.dwBuildNumber >= 22000 {
                    "Windows 11"
                } else {
                    "Windows 10"
                }
            }
            (6, 3) => "Windows 8.1",
            (6, 2) => "Windows 8",
            (6, 1) => "Windows 7",
            (6, 0) => "Windows Vista",
            (5, 2) => "Windows Server 2003 / XP x64",
            (5, 1) => "Windows XP",
            (5, 0) => "Windows 2000",
            _ => "Windows (Unknown)",
        };

        Ok(OsInfo {
            name: name.to_string(),
            version: version_str,
            build: build_str,
            is_64bit,
        })
    }
}

fn get_cpu_count() -> Result<u32> {
    // SAFETY: system_info 为栈上默认初始化的可写结构，GetNativeSystemInfo 同步填充。
    unsafe {
        let mut system_info = SYSTEM_INFO::default();
        GetNativeSystemInfo(&mut system_info);
        if system_info.dwNumberOfProcessors == 0 {
            return Err(system_info_error(
                "GetNativeSystemInfo returned zero processors",
            ));
        }
        Ok(system_info.dwNumberOfProcessors)
    }
}

fn get_memory_info() -> Result<MemoryInfo> {
    // SAFETY: mem 为完整初始化的结构（dwLength 已设置），GlobalMemoryStatusEx 只写入该结构。
    unsafe {
        let mut mem = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            dwMemoryLoad: 0,
            ullTotalPhys: 0,
            ullAvailPhys: 0,
            ullTotalPageFile: 0,
            ullAvailPageFile: 0,
            ullTotalVirtual: 0,
            ullAvailVirtual: 0,
            ullAvailExtendedVirtual: 0,
        };
        let ok = GlobalMemoryStatusEx(&mut mem);
        if ok == 0 || mem.ullTotalPhys == 0 {
            return Err(system_info_error("GlobalMemoryStatusEx"));
        }
        Ok(MemoryInfo {
            total_bytes: mem.ullTotalPhys,
            available_bytes: mem.ullAvailPhys,
            process_working_set: 0,
            process_private_bytes: 0,
        })
    }
}

fn get_hostname() -> Result<String> {
    // SAFETY: buf 为栈上可写缓冲，len 初始化为缓冲容量，GetComputerNameW 按返回的 len 写入 NUL 结尾文本。
    unsafe {
        let mut buf = [0u16; MAX_COMPUTERNAME_LENGTH + 1];
        let mut len = buf.len() as u32;
        let ok = GetComputerNameW(buf.as_mut_ptr(), &mut len);
        if ok != 0 {
            Ok(to_utf8(&buf[..len as usize]))
        } else {
            Err(system_info_error("GetComputerNameW"))
        }
    }
}

fn get_username() -> Result<String> {
    // SAFETY: buf 为栈上可写缓冲，len 初始化为缓冲容量，GetUserNameW 按返回的 len 写入 NUL 结尾文本。
    unsafe {
        let mut buf = [0u16; UNLEN + 1];
        let mut len = buf.len() as u32;
        let ok = GetUserNameW(buf.as_mut_ptr(), &mut len);
        if ok != 0 {
            Ok(to_utf8(&buf[..len as usize]))
        } else {
            Err(system_info_error("GetUserNameW"))
        }
    }
}

fn get_uptime_ms() -> Result<u64> {
    // SAFETY: GetTickCount64 无参数且不失败。
    Ok(unsafe { GetTickCount64() })
}

fn system_info_error(operation: &str) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("WindowsSystemInfo: {operation} failed"),
    )
}

// ════════════════════════════════════════════════════════════════════════════
// 数据结构定义
// ════════════════════════════════════════════════════════════════════════════

#[repr(C)]
struct RTL_OSVERSIONINFOW {
    dwOSVersionInfoSize: u32,
    dwMajorVersion: u32,
    dwMinorVersion: u32,
    dwBuildNumber: u32,
    dwPlatformId: u32,
    szCSDVersion: [u16; 128],
}

#[repr(C)]
#[derive(Default)]
struct SYSTEM_INFO {
    // Anonymous union: first member is a struct with wProcessorArchitecture
    wProcessorArchitecture: u16,
    wReserved: u16,
    dwPageSize: u32,
    lpMinimumApplicationAddress: *mut std::ffi::c_void,
    lpMaximumApplicationAddress: *mut std::ffi::c_void,
    dwActiveProcessorMask: usize,
    dwNumberOfProcessors: u32,
    dwProcessorType: u32,
    dwAllocationGranularity: u32,
    wProcessorLevel: u16,
    wProcessorRevision: u16,
}

#[repr(C)]
struct PROCESS_MEMORY_COUNTERS {
    cb: u32,
    PageFaultCount: u32,
    PeakWorkingSetSize: usize,
    WorkingSetSize: usize,
    QuotaPeakPagedPoolUsage: usize,
    QuotaPagedPoolUsage: usize,
    QuotaPeakNonPagedPoolUsage: usize,
    QuotaNonPagedPoolUsage: usize,
    PagefileUsage: usize,
    PeakPagefileUsage: usize,
    PrivateUsage: usize,
}

#[repr(C)]
struct MEMORYSTATUSEX {
    dwLength: u32,
    dwMemoryLoad: u32,
    ullTotalPhys: u64,
    ullAvailPhys: u64,
    ullTotalPageFile: u64,
    ullAvailPageFile: u64,
    ullTotalVirtual: u64,
    ullAvailVirtual: u64,
    ullAvailExtendedVirtual: u64,
}

// ════════════════════════════════════════════════════════════════════════════
// Constants
// ════════════════════════════════════════════════════════════════════════════

const MAX_COMPUTERNAME_LENGTH: usize = 31;
const UNLEN: usize = 256;

// ════════════════════════════════════════════════════════════════════════════
// Raw FFI
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "ntdll")]
// SAFETY: RtlGetVersion 声明对应 ntdll ABI，调用方提供尺寸字段正确的可写版本结构。
unsafe extern "system" {
    fn RtlGetVersion(lpVersionInformation: *mut RTL_OSVERSIONINFOW) -> i32;
}

#[link(name = "kernel32")]
// SAFETY: 本块声明对应 kernel32 ABI，调用方保证结构尺寸、缓冲区容量与输出指针有效。
unsafe extern "system" {
    fn GetNativeSystemInfo(lpSystemInfo: *mut SYSTEM_INFO);
    fn GlobalMemoryStatusEx(lpBuffer: *mut MEMORYSTATUSEX) -> i32;
    fn GetComputerNameW(lpBuffer: *mut u16, nSize: *mut u32) -> i32;
    fn GetTickCount64() -> u64;
}

#[link(name = "advapi32")]
// SAFETY: GetUserNameW 声明对应 advapi32 ABI，调用方提供与长度字段一致的可写 UTF-16 缓冲区。
unsafe extern "system" {
    fn GetUserNameW(lpBuffer: *mut u16, nSize: *mut u32) -> i32;
}

#[link(name = "psapi")]
// SAFETY: GetProcessMemoryInfo 声明对应 psapi ABI，调用方提供有效进程句柄及尺寸正确的可写计数结构。
unsafe extern "system" {
    fn GetProcessMemoryInfo(
        hProcess: *mut std::ffi::c_void,
        ppmem_counters: *mut PROCESS_MEMORY_COUNTERS,
        cb: u32,
    ) -> i32;
}

/// 获取当前进程的内存使用统计（工作集字节, 私有字节）。
// SAFETY: pmc 为 MaybeUninit 零初始化且容量正确；h_process 为 GetCurrentProcess 返回的伪句柄；assume_init 仅在 GetProcessMemoryInfo 成功返回后执行。
pub(crate) fn get_process_memory() -> Result<(usize, usize)> {
    unsafe {
        let mut pmc = std::mem::MaybeUninit::<PROCESS_MEMORY_COUNTERS>::zeroed();
        let h_process = GetCurrentProcess();
        let ret = GetProcessMemoryInfo(
            h_process,
            pmc.as_mut_ptr(),
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        );
        if ret != 0 {
            let pmc = pmc.assume_init();
            Ok((pmc.WorkingSetSize, pmc.PrivateUsage))
        } else {
            Err(system_info_error("GetProcessMemoryInfo"))
        }
    }
}

#[link(name = "kernel32")]
// SAFETY: GetCurrentProcess 声明对应 kernel32 ABI，返回值是无需关闭且仅供当前进程 API 使用的伪句柄。
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
}
