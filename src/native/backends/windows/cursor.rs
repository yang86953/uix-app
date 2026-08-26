// ============================================================================
// native/backends/windows/cursor.rs — Windows cursor (ICursor)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]

use crate::core::Point;
use crate::native::{Errc, Error, Result};
use crate::platform::windowing::{CursorType, ICursor};
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

// Windows 光标后端只在 crate 内部平台注册表中构造。
pub(crate) struct WindowsCursor {
    hwnd: *mut std::ffi::c_void,
}

impl WindowsCursor {
    // 创建未绑定窗口的光标后端。
    pub(crate) fn new() -> Self {
        Self {
            hwnd: ptr::null_mut(),
        }
    }

    // 绑定当前平台窗口句柄。
    pub(crate) fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }
}

impl Default for WindowsCursor {
    fn default() -> Self {
        Self::new()
    }
}

impl ICursor for WindowsCursor {
    fn set_cursor(&mut self, cursor: CursorType) -> Result<()> {
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
            CursorType::Custom => return Ok(()),
        };
        // SAFETY: id 是预定义系统光标标识符，hcursor 由系统分配并保持有效。
        let hcursor = unsafe { LoadCursorW(ptr::null_mut(), id as *const u16) };
        if hcursor.is_null() {
            return Err(cursor_error("LoadCursorW"));
        }
        // SAFETY: hcursor 来自 LoadCursorW，SetCursor 接受该句柄并保持其有效。
        unsafe {
            SetCursor(hcursor);
        }
        Ok(())
    }

    fn show_cursor(&mut self, visible: bool) -> Result<()> {
        // SAFETY: ShowCursor 接受布尔显示计数增量；返回值小于 0 表示失败。
        let result = unsafe { ShowCursor(if visible { TRUE } else { FALSE }) };
        if result < 0 {
            Err(cursor_error("ShowCursor"))
        } else {
            Ok(())
        }
    }

    fn cursor_position(&self) -> Result<Point> {
        // SAFETY: pt 由 GetCursorPos 在同步调用期间写入。
        let mut pt = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut pt) } != 0 {
            Ok(Point::new(pt.x as f32, pt.y as f32))
        } else {
            Err(cursor_error("GetCursorPos"))
        }
    }

    fn set_cursor_position(&mut self, x: i32, y: i32) -> Result<()> {
        // SAFETY: SetCursorPos 为同步 Win32 调用，无借用期。
        if unsafe { SetCursorPos(x, y) } != 0 {
            Ok(())
        } else {
            Err(cursor_error("SetCursorPos"))
        }
    }

    fn confine_cursor(&mut self, confine: bool) -> Result<()> {
        if confine {
            let Some(rect) = client_area_screen_rect(self.hwnd) else {
                return Err(cursor_error("GetClientRect/ClientToScreen"));
            };
            // SAFETY: rect 是当前客户区的有效 screen-space physical 矩形。
            if unsafe { ClipCursor(&rect) } != 0 {
                Ok(())
            } else {
                Err(cursor_error("ClipCursor"))
            }
        } else {
            // SAFETY: 空指针按 Win32 契约解除光标约束。
            if unsafe { ClipCursor(ptr::null()) } != 0 {
                Ok(())
            } else {
                Err(cursor_error("ClipCursor"))
            }
        }
    }

    fn capture_mouse(&mut self) -> Result<()> {
        if self.hwnd.is_null() {
            return Err(Error::new(
                Errc::InvalidState,
                "WindowsCursor::capture_mouse: no window handle bound",
            ));
        }
        // SAFETY: hwnd 来自存活平台窗口；SetCapture 返回前一个捕获窗口或空指针。
        if unsafe { SetCapture(self.hwnd) }.is_null() {
            Err(cursor_error("SetCapture"))
        } else {
            Ok(())
        }
    }

    fn release_mouse(&mut self) -> Result<()> {
        // SAFETY: ReleaseCapture 为同步 Win32 调用；返回 0 表示失败。
        if unsafe { ReleaseCapture() } != 0 {
            Ok(())
        } else {
            Err(cursor_error("ReleaseCapture"))
        }
    }
}

fn cursor_error(operation: &str) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("WindowsCursor: {operation} failed"),
    )
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
