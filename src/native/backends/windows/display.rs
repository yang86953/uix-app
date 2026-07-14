// ============================================================================
// native/backends/windows/display.rs — Windows display info (IDisplay)
// ============================================================================

#![cfg(windows)]

use super::ffi::*;
use super::util::to_wide;
use crate::core::Rect;
use crate::native::traits::display::DisplayInfo;
use crate::native::traits::display::IDisplay;
use std::cell::Cell;
use std::ptr;

pub struct WindowsDisplay {
    hwnd: Cell<usize>,
}

impl WindowsDisplay {
    pub fn new() -> Self {
        Self { hwnd: Cell::new(0) }
    }

    pub(crate) fn set_hwnd(&self, hwnd: *mut std::ffi::c_void) {
        self.hwnd.set(hwnd as usize);
    }

    /// 读取 Windows 注册表检测系统深色/浅色模式
    fn detect_os_theme() -> bool {
        unsafe {
            let sub_key =
                to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
            let value_name = to_wide("AppsUseLightTheme");
            let mut hkey: *mut std::ffi::c_void = ptr::null_mut();

            let ret = RegOpenKeyExW(
                hkey_current_user(),
                sub_key.as_ptr(),
                0,
                KEY_READ,
                &mut hkey,
            );
            if ret != ERROR_SUCCESS || hkey.is_null() {
                return false;
            }

            let mut data: u32 = 0;
            let mut data_size: u32 = std::mem::size_of::<u32>() as u32;
            let mut data_type: u32 = 0;

            let ret = RegQueryValueExW(
                hkey,
                value_name.as_ptr(),
                ptr::null_mut(),
                &mut data_type,
                &mut data as *mut u32 as *mut u8,
                &mut data_size,
            );

            RegCloseKey(hkey);

            ret == ERROR_SUCCESS && data_type == REG_DWORD && data == 0
        }
    }
}

impl Default for WindowsDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl IDisplay for WindowsDisplay {
    fn dpi_scale(&self) -> f32 {
        let hwnd = self.hwnd.get() as *mut std::ffi::c_void;
        let dpi = if hwnd.is_null() {
            super::dpi::dpi_for_system()
        } else {
            super::dpi::dpi_for_window(hwnd)
        };
        dpi as f32 / super::dpi::BASE_DPI as f32
    }

    fn is_dark_mode(&self) -> bool {
        Self::detect_os_theme()
    }

    fn count(&self) -> i32 {
        1
    }

    fn info(&self, _index: i32) -> DisplayInfo {
        unsafe {
            let w = GetSystemMetrics(SM_CXSCREEN);
            let h = GetSystemMetrics(SM_CYSCREEN);
            let dpi_scale = self.dpi_scale();
            DisplayInfo {
                bounds: Rect::new(0.0, 0.0, w as f32, h as f32),
                dpi_scale,
                is_primary: true,
            }
        }
    }
}

const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
