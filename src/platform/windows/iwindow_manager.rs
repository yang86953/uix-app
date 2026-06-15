// ============================================================================
// uix-platform/src/windows/iwindow_manager.rs — IWindowManager 实现
// ============================================================================

use super::bindings::{FLASHWINFO, FLASHW_ALL, FLASHW_TIMERNOFG, RECT};
use super::consts::*;
use super::ffi::*;
use super::gdi_presenter::GdiPresenter;
use super::platform::WindowsPlatform;
use super::util::to_wide;
use crate::diag::{Errc, Error};
use crate::platform::*;

use std::ptr;

impl IWindowManager for WindowsPlatform {
    fn create_window(&mut self, title: &str, width: i32, height: i32) -> Result<(), Error> {
        if self.class_atom == 0 {
            self.register_class()?;
        }

        let wide_title = to_wide(title);
        let class_name = self.class_name();
        let style = WS_OVERLAPPEDWINDOW;

        unsafe {
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            AdjustWindowRectEx(&mut rect, style, FALSE, WS_EX_APPWINDOW);
            let win_w = rect.right - rect.left;
            let win_h = rect.bottom - rect.top;

            let hwnd = CreateWindowExW(
                WS_EX_APPWINDOW,
                class_name.as_ptr(),
                wide_title.as_ptr(),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                win_w,
                win_h,
                ptr::null_mut(),
                ptr::null_mut(),
                self.hinstance,
                self as *mut WindowsPlatform as *mut std::ffi::c_void,
            );

            if hwnd.is_null() {
                return Err(Error::new(
                    Errc::WindowCreationFailed,
                    "create_window: CreateWindowExW returned null",
                ));
            }
            self.hwnd = hwnd;
            self.width = width;
            self.height = height;
            self.visible = false;

            // 将 hwnd 同步到所有需要它的子系统中
            self.clipboard_subsys.set_hwnd(hwnd);
            self.cursor_subsys.set_hwnd(hwnd);
            self.file_dialog_subsys.set_hwnd(hwnd);
            self.text_input_subsys.set_hwnd(hwnd);
            self.timer_subsys.set_hwnd(hwnd);
            self.notification_subsys.set_hwnd(hwnd);

            // 创建 GDI 呈现器（关联窗口的 DIB section）
            match GdiPresenter::new(hwnd, width, height) {
                Ok(p) => self.presenter = Box::new(p),
                Err(e) => {
                    log::warn!(
                        "WindowsPlatform: GdiPresenter creation failed ({}), display disabled",
                        e.short_what()
                    );
                }
            }
        }
        Ok(())
    }

    fn destroy_window(&mut self) {
        if !self.hwnd.is_null() {
            unsafe {
                DestroyWindow(self.hwnd);
            }
            self.hwnd = ptr::null_mut();
            self.visible = false;
        }
    }

    fn set_title(&mut self, title: &str) {
        let wide = to_wide(title);
        unsafe {
            SetWindowTextW(self.hwnd, wide.as_ptr());
        }
    }

    fn show(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOWNORMAL);
        }
        self.visible = true;
    }

    fn hide(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
        self.visible = false;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn center_on_screen(&mut self) {
        unsafe {
            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            let x = (sw - self.width) / 2;
            let y = (sh - self.height) / 2;
            self.set_position(x, y);
        }
    }

    fn raise(&mut self) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOP as *mut std::ffi::c_void,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
            );
        }
    }

    fn lower(&mut self) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_BOTTOM as *mut std::ffi::c_void,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    fn flash_window(&mut self) {
        unsafe {
            let mut fi = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd: self.hwnd,
                dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
                uCount: 0,
                dwTimeout: 0,
            };
            FlashWindowEx(&mut fi);
        }
    }

    fn set_window_icon(&mut self, icon_path: &str) {
        let wide = to_wide(icon_path);
        unsafe {
            let hicon = LoadImageW(
                self.hinstance,
                wide.as_ptr(),
                IMAGE_ICON,
                0,
                0,
                LR_LOADFROMFILE | LR_DEFAULTSIZE,
            ) as *mut std::ffi::c_void;
            if !hicon.is_null() {
                SendMessageW(self.hwnd, WM_SETICON, ICON_BIG, hicon as isize);
                SendMessageW(self.hwnd, WM_SETICON, ICON_SMALL, hicon as isize);
            }
        }
    }
}
