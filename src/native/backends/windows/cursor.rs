// ============================================================================
// native/backends/windows/cursor.rs — Windows cursor (ICursor)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]

use crate::core::Point;
use crate::native::traits::input::CursorType;
use crate::native::traits::input::ICursor;
use std::ptr;

use super::bindings::{POINT, RECT};
use super::ffi::{
    ClientToScreen, ClipCursor, GetClientRect, GetCursorPos, LoadCursorW, ReleaseCapture,
    SetCapture, SetCursor, SetCursorPos, ShowCursor,
};

pub(crate) fn client_area_screen_rect(hwnd: *mut std::ffi::c_void) -> Option<RECT> {
    if hwnd.is_null() {
        return None;
    }
    let mut client = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: hwnd 来自存活平台窗口，client 在同步调用期间有效可写。
    if unsafe { GetClientRect(hwnd, &mut client) } == 0 {
        return None;
    }
    let mut top_left = POINT {
        x: client.left,
        y: client.top,
    };
    let mut bottom_right = POINT {
        x: client.right,
        y: client.bottom,
    };
    // SAFETY: 两个点均属于刚查询的客户区，并在同步调用期间有效可写。
    if unsafe { ClientToScreen(hwnd, &mut top_left) } == 0
        || unsafe { ClientToScreen(hwnd, &mut bottom_right) } == 0
    {
        return None;
    }
    Some(RECT {
        left: top_left.x,
        top: top_left.y,
        right: bottom_right.x,
        bottom: bottom_right.y,
    })
}

pub struct WindowsCursor {
    hwnd: *mut std::ffi::c_void,
}

impl WindowsCursor {
    pub fn new() -> Self {
        Self {
            hwnd: ptr::null_mut(),
        }
    }

    pub fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }
}

impl Default for WindowsCursor {
    fn default() -> Self {
        Self::new()
    }
}

impl ICursor for WindowsCursor {
    fn set_cursor(&mut self, cursor: CursorType) {
        let id = match cursor {
            CursorType::Arrow => IDC_ARROW,
            CursorType::IBeam => IDC_IBEAM,
            CursorType::Crosshair => IDC_CROSS,
            CursorType::Hand => IDC_HAND,
            CursorType::ResizeH => IDC_SIZEWE,
            CursorType::ResizeV => IDC_SIZENS,
            CursorType::ResizeNE => IDC_SIZENESW,
            CursorType::ResizeNW => IDC_SIZENWSE,
            CursorType::Move => IDC_SIZEALL,
            CursorType::Wait => IDC_WAIT,
            CursorType::NotAllowed => IDC_NO,
            CursorType::Custom => return,
        };
        unsafe {
            let hcursor = LoadCursorW(ptr::null_mut(), id as *const u16);
            if !hcursor.is_null() {
                SetCursor(hcursor);
            }
        }
    }

    fn show_cursor(&mut self, visible: bool) {
        unsafe {
            ShowCursor(if visible { TRUE } else { FALSE });
        }
    }

    fn cursor_position(&self) -> Point {
        unsafe {
            let mut pt = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut pt) != 0 {
                Point::new(pt.x as f32, pt.y as f32)
            } else {
                Point::default()
            }
        }
    }

    fn set_cursor_position(&mut self, x: i32, y: i32) {
        unsafe {
            SetCursorPos(x, y);
        }
    }

    fn confine_cursor(&mut self, confine: bool) {
        if confine {
            if let Some(rect) = client_area_screen_rect(self.hwnd) {
                // SAFETY: rect 是当前客户区的有效 screen-space physical 矩形。
                unsafe {
                    ClipCursor(&rect);
                }
            }
        } else {
            // SAFETY: 空指针按 Win32 契约解除光标约束。
            unsafe {
                ClipCursor(ptr::null());
            }
        }
    }

    fn capture_mouse(&mut self) {
        unsafe {
            SetCapture(self.hwnd);
        }
    }

    fn release_mouse(&mut self) {
        unsafe {
            ReleaseCapture();
        }
    }
}

const IDC_ARROW: u16 = 32512;
const IDC_IBEAM: u16 = 32513;
const IDC_WAIT: u16 = 32514;
const IDC_CROSS: u16 = 32515;
const IDC_SIZEALL: u16 = 32646;
const IDC_NO: u16 = 32648;
const IDC_HAND: u16 = 32649;
const IDC_SIZENS: u16 = 32645;
const IDC_SIZEWE: u16 = 32644;
const IDC_SIZENWSE: u16 = 32642;
const IDC_SIZENESW: u16 = 32643;

const TRUE: i32 = 1;
const FALSE: i32 = 0;
