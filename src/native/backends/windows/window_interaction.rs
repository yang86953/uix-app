//! Windows 自定义非客户区移动与调整大小交互。

// 引入稳定平台错误分类与结果类型。
use crate::core::error::{Errc, Result};
// 引入平台中立的八方向窗口缩放契约。
use crate::platform::windowing::WindowResizeEdge;

// 引入非客户区标题栏与八方向 hit-test 常量。
use super::consts::{
    HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT,
    HTTOPRIGHT, WM_NCLBUTTONDOWN,
};
// 引入释放当前捕获与投递窗口消息的 Win32 窄端口。
use super::ffi::{PostMessageW, ReleaseCapture};

// 将平台中立缩放方向映射为 Win32 非客户区 hit-test 值。
const fn resize_hit_test(edge: WindowResizeEdge) -> usize {
    // 每个公共方向都必须映射为唯一且方向一致的 Win32 常量。
    match edge {
        // 上边对应 HTTOP。
        WindowResizeEdge::Top => HTTOP as usize,
        // 下边对应 HTBOTTOM。
        WindowResizeEdge::Bottom => HTBOTTOM as usize,
        // 左边对应 HTLEFT。
        WindowResizeEdge::Left => HTLEFT as usize,
        // 右边对应 HTRIGHT。
        WindowResizeEdge::Right => HTRIGHT as usize,
        // 左上角对应 HTTOPLEFT。
        WindowResizeEdge::TopLeft => HTTOPLEFT as usize,
        // 右上角对应 HTTOPRIGHT。
        WindowResizeEdge::TopRight => HTTOPRIGHT as usize,
        // 左下角对应 HTBOTTOMLEFT。
        WindowResizeEdge::BottomLeft => HTBOTTOMLEFT as usize,
        // 右下角对应 HTBOTTOMRIGHT。
        WindowResizeEdge::BottomRight => HTBOTTOMRIGHT as usize,
    }
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

// 提交指定边或角的自定义窗口缩放手势。
pub(super) fn begin_resize_drag(
    // HWND 已由 WindowsWindowOps 校验为有效窗口。
    hwnd: *mut std::ffi::c_void,
    // 公共方向只在当前同步调用中被解释。
    edge: WindowResizeEdge,
) -> Result<()> {
    // 映射后交给统一非客户区消息提交入口。
    begin_non_client_drag(hwnd, resize_hit_test(edge), "os_begin_resize_drag")
}

// 纯单元测试验证公共方向与 Win32 常量的一一映射。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/native/backends/windows/window_interaction__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
