//! Windows 自定义非客户区移动与逐帧响应式缩放交互。

use std::cell::RefCell;

// 引入稳定平台错误分类与结果类型。
use crate::core::error::{Errc, Result};
// 引入平台中立的八方向窗口缩放契约。
use crate::platform::windowing::WindowResizeEdge;

// 标题栏移动仍交给系统；缩放使用非模态 SetWindowPos 事务。
use super::consts::{
    GWL_EXSTYLE, GWL_STYLE, HTCAPTION, SWP_NOACTIVATE, SWP_NOZORDER, WM_NCLBUTTONDOWN,
};
use super::ffi::{
    GetCursorPos, GetSystemMetrics, GetWindowRect, PostMessageW, ReleaseCapture, SetCapture,
    SetWindowPos,
};

const SM_CXMINTRACK: i32 = 34;
const SM_CYMINTRACK: i32 = 35;

/// Windows 非模态缩放手势的物理屏幕坐标快照。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowsResizeDrag {
    edge: WindowResizeEdge,
    cursor_x: i32,
    cursor_y: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResizeTrackLimits {
    min_w: i32,
    min_h: i32,
    max_w: i32,
    max_h: i32,
}

// 释放客户区捕获并把非客户区主按钮动作交给 Windows 窗口管理器。
fn begin_non_client_drag(
    // HWND 已由 WindowsWindowOps 校验为有效窗口。
    hwnd: *mut std::ffi::c_void,
    // hit-test 决定本次是标题栏移动还是某一方向缩放。
    hit_test: usize,
    // 操作名进入稳定平台错误诊断。
    operation: &str,
) -> Result<()> {
    // SAFETY: hwnd 已由调用方校验存活；ReleaseCapture 与 PostMessageW 均为同步无指针调用。
    unsafe {
        // 原生非客户区循环接管当前主按钮手势。
        ReleaseCapture();
        // 投递与真实非客户区按下等价的窗口消息。
        if PostMessageW(hwnd, WM_NCLBUTTONDOWN, hit_test, 0) == 0 {
            // 为失败保留具体交互阶段。
            let context = format!("{operation}: PostMessageW failed");
            // 返回带 Win32 last-error 的平台错误。
            return Err(super::util::windows_diag(Errc::PlatformError, &context));
        }
    }
    // 消息已成功交付给窗口队列。
    Ok(())
}

// 提交自定义标题栏移动手势。
pub(super) fn begin_move_drag(hwnd: *mut std::ffi::c_void) -> Result<()> {
    // 标题栏 hit-test 让系统进入原生窗口移动循环。
    begin_non_client_drag(hwnd, HTCAPTION, "os_begin_move_drag")
}

// 开始指定边或角的非模态窗口缩放手势。
pub(super) fn begin_resize_drag(
    hwnd: *mut std::ffi::c_void,
    edge: WindowResizeEdge,
    drag: &RefCell<Option<WindowsResizeDrag>>,
) -> Result<()> {
    let mut cursor = super::bindings::POINT { x: 0, y: 0 };
    let mut rect = super::bindings::RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: HWND 已验证；两个栈上结构在同步调用期间保持有效可写。
    unsafe {
        if GetCursorPos(&mut cursor) == 0 || GetWindowRect(hwnd, &mut rect) == 0 {
            return Err(super::util::windows_diag(
                Errc::PlatformError,
                "os_begin_resize_drag: read cursor/window geometry failed",
            ));
        }
        // 指针越过窗口边界后仍要继续收到移动与释放消息。
        SetCapture(hwnd);
    }
    drag.replace(Some(WindowsResizeDrag {
        edge,
        cursor_x: cursor.x,
        cursor_y: cursor.y,
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }));
    Ok(())
}

/// 把当前屏幕指针应用到活动缩放事务；没有活动手势时返回 false。
pub(super) fn update_resize_drag(
    hwnd: *mut std::ffi::c_void,
    drag: &RefCell<Option<WindowsResizeDrag>>,
    minimum: Option<(i32, i32)>,
    maximum: Option<(i32, i32)>,
) -> Result<bool> {
    let Some(drag) = *drag.borrow() else {
        return Ok(false);
    };
    let mut cursor = super::bindings::POINT { x: 0, y: 0 };
    // SAFETY: cursor 是当前同步调用唯一拥有的可写 POINT。
    if unsafe { GetCursorPos(&mut cursor) } == 0 {
        return Err(super::util::windows_diag(
            Errc::PlatformError,
            "live resize: GetCursorPos failed",
        ));
    }
    let limits = resize_track_limits(hwnd, minimum, maximum)?;
    let rect = resize_drag_rect(drag, cursor.x, cursor.y, limits);
    let width = rect.right.saturating_sub(rect.left).max(1);
    let height = rect.bottom.saturating_sub(rect.top).max(1);
    // SAFETY: HWND 存活；几何已限制为正尺寸，调用不改变 z-order 或激活状态。
    if unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            rect.left,
            rect.top,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    } == 0
    {
        return Err(super::util::windows_diag(
            Errc::PlatformError,
            "live resize: SetWindowPos failed",
        ));
    }
    Ok(true)
}

/// 正常结束活动缩放并释放鼠标捕获。
pub(super) fn finish_resize_drag(drag: &RefCell<Option<WindowsResizeDrag>>) -> bool {
    if drag.replace(None).is_none() {
        return false;
    }
    // SAFETY: 活动手势由本组件建立捕获；失败只表示捕获已由系统撤销。
    unsafe {
        ReleaseCapture();
    }
    true
}

/// 异常或捕获切换时只清除事务，不重复操作系统捕获所有权。
pub(super) fn cancel_resize_drag(drag: &RefCell<Option<WindowsResizeDrag>>) {
    drag.replace(None);
}

fn resize_track_limits(
    hwnd: *mut std::ffi::c_void,
    minimum: Option<(i32, i32)>,
    maximum: Option<(i32, i32)>,
) -> Result<ResizeTrackLimits> {
    // SAFETY: GetSystemMetrics 是无指针的同步查询。
    let mut min_w = unsafe { GetSystemMetrics(SM_CXMINTRACK) }.max(1);
    let mut min_h = unsafe { GetSystemMetrics(SM_CYMINTRACK) }.max(1);
    let mut max_w = i32::MAX;
    let mut max_h = i32::MAX;
    if minimum.is_some() || maximum.is_some() {
        let style = super::window_ops::get_window_long_checked(
            hwnd,
            GWL_STYLE,
            "live resize: GetWindowLongW(GWL_STYLE) failed",
        )? as u32;
        let ex_style = super::window_ops::get_window_long_checked(
            hwnd,
            GWL_EXSTYLE,
            "live resize: GetWindowLongW(GWL_EXSTYLE) failed",
        )? as u32;
        let dpi = super::dpi::dpi_for_window(hwnd);
        if let Some((width, height)) = minimum {
            let (width, height) =
                super::dpi::outer_size_for_logical_client(width, height, style, ex_style, dpi)?;
            min_w = min_w.max(width);
            min_h = min_h.max(height);
        }
        if let Some((width, height)) = maximum {
            let (width, height) =
                super::dpi::outer_size_for_logical_client(width, height, style, ex_style, dpi)?;
            max_w = width.max(min_w);
            max_h = height.max(min_h);
        }
    }
    Ok(ResizeTrackLimits {
        min_w,
        min_h,
        max_w,
        max_h,
    })
}

fn resize_drag_rect(
    drag: WindowsResizeDrag,
    cursor_x: i32,
    cursor_y: i32,
    limits: ResizeTrackLimits,
) -> super::bindings::RECT {
    let dx = cursor_x.saturating_sub(drag.cursor_x);
    let dy = cursor_y.saturating_sub(drag.cursor_y);
    let moves_left = matches!(
        drag.edge,
        WindowResizeEdge::Left | WindowResizeEdge::TopLeft | WindowResizeEdge::BottomLeft
    );
    let moves_right = matches!(
        drag.edge,
        WindowResizeEdge::Right | WindowResizeEdge::TopRight | WindowResizeEdge::BottomRight
    );
    let moves_top = matches!(
        drag.edge,
        WindowResizeEdge::Top | WindowResizeEdge::TopLeft | WindowResizeEdge::TopRight
    );
    let moves_bottom = matches!(
        drag.edge,
        WindowResizeEdge::Bottom | WindowResizeEdge::BottomLeft | WindowResizeEdge::BottomRight
    );
    let mut left = if moves_left {
        drag.left.saturating_add(dx)
    } else {
        drag.left
    };
    let mut right = if moves_right {
        drag.right.saturating_add(dx)
    } else {
        drag.right
    };
    let mut top = if moves_top {
        drag.top.saturating_add(dy)
    } else {
        drag.top
    };
    let mut bottom = if moves_bottom {
        drag.bottom.saturating_add(dy)
    } else {
        drag.bottom
    };
    clamp_axis(
        &mut left,
        &mut right,
        moves_left,
        limits.min_w,
        limits.max_w,
    );
    clamp_axis(&mut top, &mut bottom, moves_top, limits.min_h, limits.max_h);
    super::bindings::RECT {
        left,
        top,
        right,
        bottom,
    }
}

fn clamp_axis(start: &mut i32, end: &mut i32, moves_start: bool, min: i32, max: i32) {
    let extent = end.saturating_sub(*start).clamp(min.max(1), max.max(min));
    if moves_start {
        *start = end.saturating_sub(extent);
    } else {
        *end = start.saturating_add(extent);
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/native/backends/windows/window_interaction__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
