// ============================================================================
// native/backends/windows/filesystem.rs — Windows 特殊目录解析
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]

use crate::native::backends::windows::util::to_utf8;
use crate::native::capabilities::services::{FileSystemCore, SpecialDirProvider};
use crate::native::capabilities::system::SpecialDir;
use crate::native::{Errc, Error, Result};
use std::ptr;

// ════════════════════════════════════════════════════════════════════════════
// WindowsFileSystem
// ════════════════════════════════════════════════════════════════════════════

pub type WindowsFileSystem = FileSystemCore<WindowsSpecialDirs>;

#[derive(Debug, Clone, Default)]
pub struct WindowsSpecialDirs;

impl SpecialDirProvider for WindowsSpecialDirs {
    fn special_dir(&self, dir: SpecialDir) -> Result<String> {
        match dir {
            SpecialDir::Temp => get_temp_dir(),
            _ => {
                // 已知目录交给 Shell API，其他公共目录由 FileSystemCore 处理。
                let guid = match dir {
                    SpecialDir::Home => FOLDERID_PROFILE,
                    SpecialDir::AppData => FOLDERID_ROAMING_APP_DATA,
                    SpecialDir::LocalAppData => FOLDERID_LOCAL_APP_DATA,
                    SpecialDir::Documents => FOLDERID_DOCUMENTS,
                    SpecialDir::Desktop => FOLDERID_DESKTOP,
                    SpecialDir::Downloads => FOLDERID_DOWNLOADS,
                    _ => {
                        return Err(Error::new(
                            Errc::InvalidArgument,
                            format!("WindowsSpecialDirs::special_dir: unsupported dir {dir:?}"),
                        ));
                    }
                };
                get_known_folder_path(&guid)
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 内部实现函数
// ════════════════════════════════════════════════════════════════════════════

fn get_temp_dir() -> Result<String> {
    read_variable_wide_path(|buffer| unsafe {
        GetTempPathW(buffer.len() as u32, buffer.as_mut_ptr()) as usize
    })
}

pub(crate) fn read_variable_wide_path(
    mut query: impl FnMut(&mut [u16]) -> usize,
) -> Result<String> {
    let mut buffer = vec![0u16; MAX_PATH + 1];
    loop {
        let length = query(&mut buffer);
        if length == 0 {
            return Err(Error::new(
                Errc::PlatformError,
                "WindowsSpecialDirs: Win32 path query returned zero length",
            ));
        }
        if length < buffer.len() {
            return Ok(to_utf8(&buffer[..length]));
        }

        let Some(next_capacity) = length.checked_add(1) else {
            return Err(Error::new(
                Errc::OutOfRange,
                "WindowsSpecialDirs: path length overflow",
            ));
        };
        if next_capacity > MAX_WIN32_PATH_UNITS {
            return Err(Error::new(
                Errc::OutOfRange,
                "WindowsSpecialDirs: path exceeds MAX_WIN32_PATH_UNITS",
            ));
        }
        buffer.resize(next_capacity, 0);
    }
}

fn get_known_folder_path(guid: &GUID) -> Result<String> {
    unsafe {
        let mut path_ptr: *mut u16 = ptr::null_mut();
        let hr = SHGetKnownFolderPath(guid as *const GUID, 0, ptr::null_mut(), &mut path_ptr);
        if hr >= 0 && !path_ptr.is_null() {
            let mut len = 0;
            while *path_ptr.add(len) != 0 {
                len += 1;
            }
            let result = to_utf8(std::slice::from_raw_parts(path_ptr, len));
            CoTaskMemFree(path_ptr as *mut std::ffi::c_void);
            Ok(result)
        } else {
            Err(Error::new(
                Errc::PlatformError,
                format!(
                    "WindowsSpecialDirs: SHGetKnownFolderPath failed with HRESULT {:#x}",
                    hr as u32
                ),
            ))
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// SHGetKnownFolderPath 使用的 GUID 定义
// ════════════════════════════════════════════════════════════════════════════

#[repr(C)]
struct GUID {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

// Windows 已知目录 GUID
const FOLDERID_PROFILE: GUID = GUID {
    data1: 0x5E6C858F,
    data2: 0x0E22,
    data3: 0x4760,
    data4: [0x9A, 0xFE, 0xEA, 0x33, 0x17, 0xB6, 0x73, 0x73],
};

const FOLDERID_ROAMING_APP_DATA: GUID = GUID {
    data1: 0x3EB685DB,
    data2: 0x65F9,
    data3: 0x4CF6,
    data4: [0xA0, 0x3A, 0xE3, 0xEF, 0x65, 0x72, 0x9F, 0x3D],
};

const FOLDERID_LOCAL_APP_DATA: GUID = GUID {
    data1: 0xF1B32785,
    data2: 0x6FBA,
    data3: 0x4FCF,
    data4: [0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91],
};

const FOLDERID_DOCUMENTS: GUID = GUID {
    data1: 0xFDD39AD0,
    data2: 0x238F,
    data3: 0x46AF,
    data4: [0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7],
};

const FOLDERID_DESKTOP: GUID = GUID {
    data1: 0xB4BFCC3A,
    data2: 0xDB2C,
    data3: 0x424C,
    data4: [0xB0, 0x29, 0x7F, 0xE9, 0x9A, 0x87, 0xC6, 0x41],
};

const FOLDERID_DOWNLOADS: GUID = GUID {
    data1: 0x374DE290,
    data2: 0x123F,
    data3: 0x4565,
    data4: [0x91, 0x64, 0x39, 0xC4, 0x92, 0x5E, 0x46, 0x7B],
};

// ════════════════════════════════════════════════════════════════════════════
// 常量
// ════════════════════════════════════════════════════════════════════════════

const MAX_PATH: usize = 260;
const MAX_WIN32_PATH_UNITS: usize = 32_768;

// ════════════════════════════════════════════════════════════════════════════
// 原始 FFI
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetTempPathW(nBufferLength: u32, lpBuffer: *mut u16) -> u32;
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoTaskMemFree(pv: *mut std::ffi::c_void);
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(
        rfid: *const GUID,
        dwFlags: u32,
        hToken: *mut std::ffi::c_void,
        ppszPath: *mut *mut u16,
    ) -> i32;
}
