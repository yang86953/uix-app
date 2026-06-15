// ============================================================================
// uix-platform/src/windows/system_info.rs — Windows system info impl (ISystemInfo)
// ============================================================================

#![cfg(windows)]
#![allow(nonstandard_style)]
#![allow(clippy::upper_case_acronyms)]

use crate::platform::windows::util::to_utf8;
use crate::platform::{ISystemInfo, MemoryInfo, OsInfo};

// ════════════════════════════════════════════════════════════════════════════
// WindowsSystemInfo
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct WindowsSystemInfo;

impl WindowsSystemInfo {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsSystemInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ISystemInfo for WindowsSystemInfo {
    fn os_info(&self) -> OsInfo {
        get_os_info()
    }

    fn cpu_count(&self) -> u32 {
        get_cpu_count()
    }

    fn memory_info(&self) -> MemoryInfo {
        get_memory_info()
    }

    fn hostname(&self) -> String {
        get_hostname()
    }

    fn username(&self) -> String {
        get_username()
    }

    fn up_time(&self) -> u64 {
        get_uptime_ms()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 内部实现
// ════════════════════════════════════════════════════════════════════════════

fn get_os_info() -> OsInfo {
    // Use RtlGetVersion to get accurate OS version (not affected by manifest compat)
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
        if status == 0 {
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

            OsInfo {
                name: name.to_string(),
                version: version_str,
                build: build_str,
                is_64bit,
            }
        } else {
            OsInfo {
                name: "Windows (Unknown)".to_string(),
                version: String::new(),
                build: String::new(),
                is_64bit: false,
            }
        }
    }
}

fn get_cpu_count() -> u32 {
    unsafe {
        let mut system_info = SYSTEM_INFO::default();
        GetNativeSystemInfo(&mut system_info);
        system_info.dwNumberOfProcessors
    }
}

fn get_memory_info() -> MemoryInfo {
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
        if ok != 0 {
            MemoryInfo {
                total_bytes: mem.ullTotalPhys,
                available_bytes: mem.ullAvailPhys,
            }
        } else {
            MemoryInfo {
                total_bytes: 0,
                available_bytes: 0,
            }
        }
    }
}

fn get_hostname() -> String {
    unsafe {
        let mut buf = [0u16; MAX_COMPUTERNAME_LENGTH + 1];
        let mut len = buf.len() as u32;
        let ok = GetComputerNameW(buf.as_mut_ptr(), &mut len);
        if ok != 0 {
            to_utf8(&buf[..len as usize])
        } else {
            String::new()
        }
    }
}

fn get_username() -> String {
    unsafe {
        let mut buf = [0u16; UNLEN + 1];
        let mut len = buf.len() as u32;
        let ok = GetUserNameW(buf.as_mut_ptr(), &mut len);
        if ok != 0 {
            to_utf8(&buf[..len as usize])
        } else {
            String::new()
        }
    }
}

fn get_uptime_ms() -> u64 {
    unsafe { GetTickCount64() }
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
extern "system" {
    fn RtlGetVersion(lpVersionInformation: *mut RTL_OSVERSIONINFOW) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetNativeSystemInfo(lpSystemInfo: *mut SYSTEM_INFO);
    fn GlobalMemoryStatusEx(lpBuffer: *mut MEMORYSTATUSEX) -> i32;
    fn GetComputerNameW(lpBuffer: *mut u16, nSize: *mut u32) -> i32;
    fn GetTickCount64() -> u64;
}

#[link(name = "advapi32")]
extern "system" {
    fn GetUserNameW(lpBuffer: *mut u16, nSize: *mut u32) -> i32;
}
