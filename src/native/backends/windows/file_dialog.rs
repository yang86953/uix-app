// ============================================================================
// native/backends/windows/file_dialog.rs — Windows file dialog (IFileDialog)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]
#![allow(nonstandard_style)]

use crate::native::backends::windows::util::{to_utf8, to_wide};
use crate::native::{Errc, Error, Result};
use std::ptr;

/// `GetOpenFileNameW` / `GetSaveFileNameW` 返回 0 时,0 表示用户取消,
/// 非 0 表示对话框内部失败(如内存不足、模板无效)。
fn dialog_failure_or_cancelled() -> Result<()> {
    // SAFETY: CommDlgExtendedError 无参数,返回最近一次对话框错误的扩展码。
    let error = unsafe { CommDlgExtendedError() };
    if error == 0 {
        Ok(())
    } else {
        Err(Error::new(
            Errc::PlatformError,
            format!("WindowsFileDialog: common dialog failed with extended error {error}"),
        ))
    }
}

// 打开允许多选的文件面板；对话框不绑定父窗口。
fn open_files(title: &str, filters: &str) -> Result<Option<Vec<String>>> {
    let wide_filters = to_wide(filters);
    let mut buf = [0u16; 4096];
    let wide_title = to_wide(title);
    // SAFETY: OPENFILENAMEW 中的所有指针均指向本作用域内存活且可写范围已知的缓冲区，系统调用返回后才解析结果。
    unsafe {
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: ptr::null_mut(),
            hInstance: ptr::null_mut(),
            lpstrFilter: wide_filters.as_ptr(),
            lpstrCustomFilter: ptr::null_mut(),
            nMaxCustFilter: 0,
            nFilterIndex: 1,
            lpstrFile: buf.as_mut_ptr(),
            nMaxFile: 4096,
            lpstrFileTitle: ptr::null_mut(),
            nMaxFileTitle: 0,
            lpstrInitialDir: ptr::null(),
            lpstrTitle: wide_title.as_ptr(),
            Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_HIDEREADONLY | OFN_ALLOWMULTISELECT,
            nFileOffset: 0,
            nFileExtension: 0,
            lpstrDefExt: ptr::null(),
            lCustData: 0,
            // Hook callback 不注册；避免把 Rust 函数指针交给 common dialog。
            lpfnHook: ptr::null_mut(),
            lpTemplateName: ptr::null(),
            pvReserved: ptr::null_mut(),
            dwReserved: 0,
            FlagsEx: 0,
        };
        let result = GetOpenFileNameW(&mut ofn);
        if result == 0 {
            return dialog_failure_or_cancelled().map(|()| None);
        }
        let wide_str = &buf[..];
        let null_pos = wide_str.iter().position(|&c| c == 0).unwrap_or(0);
        if null_pos == 0 {
            return Err(Error::new(
                Errc::FormatError,
                "WindowsFileDialog::open_files: empty file buffer",
            ));
        }
        let dir = to_utf8(&wide_str[..null_pos]);
        if dir.is_empty() {
            return Err(Error::new(
                Errc::FormatError,
                "WindowsFileDialog::open_files: empty directory path",
            ));
        }
        let remaining = &wide_str[(null_pos + 1)..];
        let second_null = remaining.iter().position(|&c| c == 0).unwrap_or(0);
        if second_null == 0 || remaining[0] == 0 {
            return Ok(Some(vec![dir]));
        }
        let mut result = Vec::new();
        let mut pos = 0;
        loop {
            if pos >= remaining.len() || remaining[pos] == 0 {
                break;
            }
            let end = remaining[pos..]
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(remaining.len() - pos);
            let file_name = to_utf8(&remaining[pos..pos + end]);
            if file_name.is_empty() {
                break;
            }
            result.push(format!("{}\\{}", dir, file_name));
            pos = pos + end + 1;
        }
        Ok(Some(result))
    }
}

// 打开单路径保存面板；对话框不绑定父窗口。
fn save_file(title: &str, filters: &str) -> Result<Option<String>> {
    let wide_filters = to_wide(filters);
    let mut buf = [0u16; 4096];
    let wide_title = to_wide(title);
    // SAFETY: OPENFILENAMEW 仅借用本作用域中的 UTF-16 缓冲区，调用期间这些缓冲区不会移动或失效。
    unsafe {
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: ptr::null_mut(),
            hInstance: ptr::null_mut(),
            lpstrFilter: wide_filters.as_ptr(),
            lpstrCustomFilter: ptr::null_mut(),
            nMaxCustFilter: 0,
            nFilterIndex: 1,
            lpstrFile: buf.as_mut_ptr(),
            nMaxFile: 4096,
            lpstrFileTitle: ptr::null_mut(),
            nMaxFileTitle: 0,
            lpstrInitialDir: ptr::null(),
            lpstrTitle: wide_title.as_ptr(),
            Flags: OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY | OFN_OVERWRITEPROMPT,
            nFileOffset: 0,
            nFileExtension: 0,
            lpstrDefExt: ptr::null(),
            lCustData: 0,
            // Hook callback 不注册；避免把 Rust 函数指针交给 common dialog。
            lpfnHook: ptr::null_mut(),
            lpTemplateName: ptr::null(),
            pvReserved: ptr::null_mut(),
            dwReserved: 0,
            FlagsEx: 0,
        };
        let result = GetSaveFileNameW(&mut ofn);
        if result == 0 {
            return dialog_failure_or_cancelled().map(|()| None);
        }
        Ok(Some(to_utf8(&buf)))
    }
}

// 打开只允许单选目录的面板；对话框不绑定父窗口。
fn open_folder(title: &str) -> Result<Option<String>> {
    let wide_title = to_wide(title);
    let mut buf = [0u16; 4096];
    // SAFETY: BROWSEINFOW 的输入与输出缓冲区在同步调用期间保持有效，返回的 PIDL 最终由 CoTaskMemFree 释放。
    unsafe {
        let mut bi = BROWSEINFOW {
            hwndOwner: ptr::null_mut(),
            pidlRoot: ptr::null_mut(),
            pszDisplayName: buf.as_mut_ptr(),
            lpszTitle: wide_title.as_ptr(),
            ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
            // Folder-picker callback 不注册；这里没有 Rust ABI callback owner。
            lpfn: None,
            lParam: 0,
            iImage: 0,
        };
        let pidl = SHBrowseForFolderW(&mut bi);
        if pidl.is_null() {
            return Ok(None);
        }
        let result = SHGetPathFromIDListW(pidl, buf.as_mut_ptr());
        CoTaskMemFree(pidl);
        if result != 0 {
            Ok(Some(to_utf8(&buf)))
        } else {
            Err(Error::new(
                Errc::PlatformError,
                "WindowsFileDialog::open_folder: SHGetPathFromIDListW failed",
            ))
        }
    }
}

// 通过公开 Platform System 的窄 Adapter 打开多选文件面板。
pub(crate) fn choose_files(title: &str, filters: &str) -> Result<Option<Vec<String>>> {
    open_files(title, filters)
}

// 通过公开 Platform System 的窄 Adapter 打开保存面板。
pub(crate) fn choose_save_file(title: &str, filters: &str) -> Result<Option<String>> {
    save_file(title, filters)
}

// 通过公开 Platform System 的窄 Adapter 打开目录面板。
pub(crate) fn choose_folder(title: &str) -> Result<Option<String>> {
    open_folder(title)
}

// ── FFI ──

#[repr(C)]
struct OPENFILENAMEW {
    lStructSize: u32,
    hwndOwner: *mut std::ffi::c_void,
    hInstance: *mut std::ffi::c_void,
    lpstrFilter: *const u16,
    lpstrCustomFilter: *mut u16,
    nMaxCustFilter: u32,
    nFilterIndex: u32,
    lpstrFile: *mut u16,
    nMaxFile: u32,
    lpstrFileTitle: *mut u16,
    nMaxFileTitle: u32,
    lpstrInitialDir: *const u16,
    lpstrTitle: *const u16,
    Flags: u32,
    nFileOffset: u16,
    nFileExtension: u16,
    lpstrDefExt: *const u16,
    lCustData: isize,
    lpfnHook: *mut std::ffi::c_void,
    lpTemplateName: *const u16,
    pvReserved: *mut std::ffi::c_void,
    dwReserved: u32,
    FlagsEx: u32,
}

const OFN_EXPLORER: u32 = 0x00080000;
const OFN_FILEMUSTEXIST: u32 = 0x00001000;
const OFN_HIDEREADONLY: u32 = 0x00000004;
const OFN_ALLOWMULTISELECT: u32 = 0x00000200;
const OFN_PATHMUSTEXIST: u32 = 0x00000800;
const OFN_OVERWRITEPROMPT: u32 = 0x00000002;

#[link(name = "comdlg32")]
// SAFETY: 这些声明严格对应 comdlg32 的 Win32 ABI，调用方负责提供有效且尺寸正确的 OPENFILENAMEW。
unsafe extern "system" {
    fn GetOpenFileNameW(lpofn: *mut OPENFILENAMEW) -> i32;
    fn GetSaveFileNameW(lpofn: *mut OPENFILENAMEW) -> i32;
    fn CommDlgExtendedError() -> u32;
}

// ── Folder picker FFI ──

#[repr(C)]
struct BROWSEINFOW {
    hwndOwner: *mut std::ffi::c_void,
    pidlRoot: *mut std::ffi::c_void,
    pszDisplayName: *mut u16,
    lpszTitle: *const u16,
    ulFlags: u32,
    // SAFETY: 回调函数若存在必须遵守 SHBrowseForFolderW 规定的 system ABI 与参数生命周期。
    lpfn: Option<unsafe extern "system" fn(*mut BROWSEINFOW, *mut std::ffi::c_void) -> i32>,
    lParam: isize,
    iImage: i32,
}

const BIF_RETURNONLYFSDIRS: u32 = 0x0001;
const BIF_NEWDIALOGSTYLE: u32 = 0x0040;

#[link(name = "shell32")]
// SAFETY: 这些声明严格对应 shell32 的 Win32 ABI，调用方负责缓冲区有效性并接管返回 PIDL 的释放责任。
unsafe extern "system" {
    fn SHBrowseForFolderW(lpbi: *mut BROWSEINFOW) -> *mut std::ffi::c_void;
    fn SHGetPathFromIDListW(pidl: *mut std::ffi::c_void, pszPath: *mut u16) -> i32;
}

#[link(name = "ole32")]
// SAFETY: CoTaskMemFree 的声明对应 ole32 ABI，仅可传入 COM 任务分配器返回的指针。
unsafe extern "system" {
    fn CoTaskMemFree(pv: *mut std::ffi::c_void);
}
