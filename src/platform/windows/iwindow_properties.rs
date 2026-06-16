// ============================================================================
// uix-platform/src/windows/iwindow_properties.rs — IWindowProperties 实现
// ============================================================================

use super::consts::*;
use super::ffi::*;
use super::platform::WindowsPlatform;
use crate::base::*;
use crate::platform::*;

use std::ptr;

impl IWindowProperties for WindowsPlatform {
    fn width(&self) -> i32 {
        self.width
    }
    fn height(&self) -> i32 {
        self.height
    }

    fn set_size(&mut self, w: i32, h: i32) {
        self.width = w;
        self.height = h;
        unsafe {
            SetWindowPos(
                self.hwnd,
                ptr::null_mut(),
                0,
                0,
                w,
                h,
                SWP_NOMOVE | SWP_NOZORDER,
            );
        }
    }

    fn set_minimum_size(&mut self, w: i32, h: i32) {
        self.min_w = w;
        self.min_h = h;
    }
    fn set_maximum_size(&mut self, w: i32, h: i32) {
        self.max_w = w;
        self.max_h = h;
    }

    fn position(&self) -> Point {
        Point::new(self.pos_x as f32, self.pos_y as f32)
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.pos_x = x;
        self.pos_y = y;
        unsafe {
            SetWindowPos(
                self.hwnd,
                ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    fn set_resizable(&mut self, resizable: bool) {
        self.resizable = resizable;
        unsafe {
            let style = GetWindowLongW(self.hwnd, GWL_STYLE) as u32;
            let new_style = if resizable {
                style | WS_THICKFRAME
            } else {
                style & !WS_THICKFRAME
            };
            SetWindowLongW(self.hwnd, GWL_STYLE, new_style as i32);
            SetWindowPos(
                self.hwnd,
                ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    fn is_maximized(&self) -> bool {
        self.maximized
    }
    fn is_minimized(&self) -> bool {
        self.minimized
    }
    fn maximize(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_MAXIMIZE);
        }
    }
    fn minimize(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_MINIMIZE);
        }
    }
    fn restore(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_RESTORE);
        }
    }

    fn set_borderless(&mut self, borderless: bool) {
        self.borderless = borderless;
        unsafe {
            let style = GetWindowLongW(self.hwnd, GWL_STYLE) as u32;
            let new_style = if borderless {
                style & !WS_OVERLAPPEDWINDOW
            } else {
                style | WS_OVERLAPPEDWINDOW
            };
            SetWindowLongW(self.hwnd, GWL_STYLE, new_style as i32);
            SetWindowPos(
                self.hwnd,
                ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        if fullscreen == self.fullscreen {
            return;
        }
        self.fullscreen = fullscreen;
        if fullscreen {
            unsafe {
                SetWindowLongW(self.hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE) as i32);
                let sw = GetSystemMetrics(SM_CXSCREEN);
                let sh = GetSystemMetrics(SM_CYSCREEN);
                SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST as *mut std::ffi::c_void,
                    0,
                    0,
                    sw,
                    sh,
                    SWP_FRAMECHANGED,
                );
            }
        } else {
            unsafe {
                let mut flags = WS_OVERLAPPEDWINDOW | WS_VISIBLE;
                if !self.resizable {
                    flags &= !WS_THICKFRAME;
                }
                SetWindowLongW(self.hwnd, GWL_STYLE, flags as i32);
                SetWindowPos(
                    self.hwnd,
                    HWND_NOTOPMOST as *mut std::ffi::c_void,
                    self.pos_x,
                    self.pos_y,
                    self.width,
                    self.height,
                    SWP_FRAMECHANGED,
                );
            }
        }
    }

    fn is_fullscreen(&self) -> bool {
        self.fullscreen
    }

    fn set_always_on_top(&mut self, on: bool) {
        self.always_on_top = on;
        unsafe {
            let pos = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
            SetWindowPos(
                self.hwnd,
                pos as *mut std::ffi::c_void,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    fn set_window_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
        if opacity < 1.0 {
            unsafe {
                let ex_style = GetWindowLongW(self.hwnd, GWL_EXSTYLE) as u32;
                SetWindowLongW(self.hwnd, GWL_EXSTYLE, (ex_style | WS_EX_LAYERED) as i32);
                SetLayeredWindowAttributes(self.hwnd, 0, (opacity * 255.0) as u8, LWA_ALPHA);
            }
        }
    }

    fn start_text_input(&mut self) {
        self.text_input_active = true;
        self.text_input_subsys.start();
    }
    fn stop_text_input(&mut self) {
        self.text_input_active = false;
        self.text_input_subsys.stop();
    }

    fn enable_file_drop(&mut self, enable: bool) {
        self.file_drop_enabled = enable;
        unsafe {
            DragAcceptFiles(self.hwnd, if enable { TRUE } else { FALSE });
        }
    }
}
