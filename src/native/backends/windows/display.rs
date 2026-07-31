// ============================================================================
// native/backends/windows/display.rs — Windows display info (IDisplay)
// ============================================================================

#![cfg(windows)]

use super::ffi::*;
use super::util::to_wide;
use crate::core::Rect;
use crate::native::capabilities::display::DisplayInfo;
use crate::native::capabilities::display::IDisplay;
use crate::native::{Errc, Error, Result};
use std::cell::Cell;
use std::ptr;
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

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
    pub(crate) fn detect_os_theme() -> Result<bool> {
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
                return Err(Error::new(
                    Errc::PlatformError,
                    format!("WindowsDisplay::is_dark_mode: RegOpenKeyExW failed ({ret})"),
                ));
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

            if ret == ERROR_SUCCESS && data_type == REG_DWORD {
                Ok(data == 0)
            } else {
                Err(Error::new(
                    Errc::PlatformError,
                    format!("WindowsDisplay::is_dark_mode: RegQueryValueExW failed ({ret})"),
                ))
            }
        }
    }

    fn monitors(&self) -> Result<Vec<MonitorDescriptor>> {
        let mut inventory = MonitorInventory::default();
        let inventory_ptr = (&mut inventory as *mut MonitorInventory) as isize;
        // SAFETY: EnumDisplayMonitors 同步调用回调，inventory 在整个调用期间唯一可写且有效。
        let completed = unsafe {
            EnumDisplayMonitors(None, None, Some(collect_monitor), LPARAM(inventory_ptr))
        };
        if !completed.as_bool() || inventory.failed {
            return Err(Error::new(
                Errc::PlatformError,
                "WindowsDisplay::monitors: EnumDisplayMonitors failed",
            ));
        }
        inventory.monitors.sort_by_key(|monitor| {
            (
                !monitor.is_primary,
                monitor.bounds.top,
                monitor.bounds.left,
                monitor.bounds.bottom,
                monitor.bounds.right,
            )
        });
        Ok(inventory.monitors)
    }

    fn dpi_for_monitor(&self, monitor: HMONITOR) -> u32 {
        let mut dpi_x = 0;
        let mut dpi_y = 0;
        // SAFETY: monitor 来自本轮 EnumDisplayMonitors；输出指针在同步调用期间有效。
        if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.is_ok()
            && dpi_x > 0
        {
            return dpi_x;
        }
        let hwnd = self.hwnd.get() as *mut std::ffi::c_void;
        if !hwnd.is_null() {
            // SAFETY: hwnd 由仍存活的平台窗口登记；只比较其所在显示器句柄。
            let selected = unsafe {
                MonitorFromWindow(
                    windows::Win32::Foundation::HWND(hwnd),
                    MONITOR_DEFAULTTONEAREST,
                )
            };
            if selected == monitor {
                return super::dpi::dpi_for_window(hwnd);
            }
        }
        super::dpi::dpi_for_system()
    }
}

#[derive(Clone, Copy)]
struct MonitorDescriptor {
    handle: HMONITOR,
    bounds: RECT,
    is_primary: bool,
}

#[derive(Default)]
struct MonitorInventory {
    monitors: Vec<MonitorDescriptor>,
    failed: bool,
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _device_context: HDC,
    _bounds: *mut RECT,
    inventory: LPARAM,
) -> BOOL {
    if inventory.0 == 0 {
        return BOOL(0);
    }
    // SAFETY: LPARAM 由 monitors() 指向当前同步枚举期间唯一的 MonitorInventory。
    let inventory = unsafe { &mut *(inventory.0 as *mut MonitorInventory) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: monitor 由系统回调提供，info 在调用期间有效可写。
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        inventory.failed = true;
        return BOOL(0);
    }
    inventory.monitors.push(MonitorDescriptor {
        handle: monitor,
        bounds: info.rcMonitor,
        is_primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
    });
    BOOL(1)
}

impl Default for WindowsDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl IDisplay for WindowsDisplay {
    fn dpi_scale(&self) -> Result<f32> {
        let hwnd = self.hwnd.get() as *mut std::ffi::c_void;
        let dpi = if hwnd.is_null() {
            super::dpi::dpi_for_system()
        } else {
            super::dpi::dpi_for_window(hwnd)
        };
        Ok(dpi as f32 / super::dpi::BASE_DPI as f32)
    }

    fn is_dark_mode(&self) -> Result<bool> {
        Self::detect_os_theme()
    }

    fn count(&self) -> Result<i32> {
        i32::try_from(self.monitors()?.len())
            .map_err(|_| Error::new(Errc::OutOfRange, "WindowsDisplay::count: too many monitors"))
    }

    fn info(&self, index: i32) -> Result<DisplayInfo> {
        let monitors = self.monitors()?;
        let monitor = monitors
            .get(index.max(0) as usize)
            .or_else(|| monitors.first())
            .ok_or_else(|| Error::new(Errc::NotFound, "WindowsDisplay::info: no monitors"))?;
        Ok(DisplayInfo {
            bounds: Rect::new(
                monitor.bounds.left as f32,
                monitor.bounds.top as f32,
                (monitor.bounds.right - monitor.bounds.left) as f32,
                (monitor.bounds.bottom - monitor.bounds.top) as f32,
            ),
            dpi_scale: self.dpi_for_monitor(monitor.handle) as f32 / super::dpi::BASE_DPI as f32,
            is_primary: monitor.is_primary,
        })
    }
}
