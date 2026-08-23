// ============================================================================
// platform/windows/wnd_proc.rs — Windows 窗口过程 + 消息处理
//
// wnd_proc: Win32 窗口过程回调（extern "system"）
// handle_message: 消息分发 → UiEvent 转换 → 状态更新
// ============================================================================

#![cfg(windows)]
#![allow(non_snake_case)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use crate::core::Point;
use crate::native::{Errc, Error};
use crate::platform::windowing::MouseButton;
use crate::platform::windowing::event::{UiEvent, UiEventPayload, UiEventType};

use super::bindings::*;
use super::consts::*;
use super::display::WindowsDisplay;
use super::ffi::*;
use super::frame_pacer::{clear_pending_frame, complete_posted_frame};
use super::ime_dispatch::{
    ImmStringRead, ime_composition_events, ime_end_composition_event, ime_start_composition_event,
};
use super::platform::{WindowBinding, WindowsPlatform};
use super::text_input::{WindowsImeState, composition_string, result_string};

fn apply_window_track_constraints(
    hwnd: *mut std::ffi::c_void,
    minimum: Option<(i32, i32)>,
    maximum: Option<(i32, i32)>,
    info: &mut MINMAXINFO,
) -> crate::native::Result<()> {
    let style = super::window_ops::get_window_long_checked(
        hwnd,
        GWL_STYLE,
        "WM_GETMINMAXINFO GetWindowLongW(GWL_STYLE) failed",
    )? as u32;
    if style & WS_POPUP != 0 {
        return Ok(());
    }
    let ex_style = super::window_ops::get_window_long_checked(
        hwnd,
        GWL_EXSTYLE,
        "WM_GETMINMAXINFO GetWindowLongW(GWL_EXSTYLE) failed",
    )? as u32;
    let dpi = super::dpi::dpi_for_window(hwnd);
    if let Some((width, height)) = minimum {
        let (outer_width, outer_height) =
            super::dpi::outer_size_for_logical_client(width, height, style, ex_style, dpi)?;
        info.ptMinTrackSize = POINT {
            x: outer_width,
            y: outer_height,
        };
    }
    if let Some((width, height)) = maximum {
        let (outer_width, outer_height) =
            super::dpi::outer_size_for_logical_client(width, height, style, ex_style, dpi)?;
        info.ptMaxTrackSize = POINT {
            x: outer_width,
            y: outer_height,
        };
    }
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════
// 窗口过程回调
// ════════════════════════════════════════════════════════════════════════════

/// Win32 窗口过程回调入口（由系统经 ABI 调用）。
///
/// # Safety
/// 由 Win32 消息循环调用：hwnd 为存活窗口句柄，msg/wparam/lparam 为当前消息参数；Rust panic 被 run_wnd_proc_boundary 捕获不会越过 ABI 边界。
pub(crate) unsafe extern "system" fn wnd_proc(
    hwnd: *mut std::ffi::c_void,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    run_wnd_proc_boundary(
        // SAFETY: hwnd/msg/wparam/lparam 为 Win32 传入的当前消息参数，wnd_proc 自身的不变量保证其有效。
        || unsafe { wnd_proc_inner(hwnd, msg, wparam, lparam) },
        // SAFETY: 同上，panic 通知路径只读取窗口绑定指针。
        || unsafe { enqueue_wnd_proc_panic(hwnd, msg) },
        // SAFETY: DefWindowProcW 接受同一组当前消息参数，且不跨越 ABI 展开。
        || unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    )
}

/// Keeps a Rust panic inside the Win32 callback boundary.
///
/// The notification and fallback are defensive boundaries too: a failure
/// while recording diagnostics must not become a second unwind across the
/// Windows ABI.
fn run_wnd_proc_boundary<Run, Notify, Fallback>(
    run: Run,
    notify: Notify,
    fallback: Fallback,
) -> isize
where
    Run: FnOnce() -> isize,
    Notify: FnOnce(),
    Fallback: FnOnce() -> isize,
{
    match catch_unwind(AssertUnwindSafe(run)) {
        Ok(result) => result,
        Err(_) => {
            let _ = catch_unwind(AssertUnwindSafe(notify));
            catch_unwind(AssertUnwindSafe(fallback)).unwrap_or(0)
        }
    }
}

/// 在 panic 越界时通知平台所有者，记录回调失败。
///
/// # Safety
/// 调用者必须保证 hwnd 存活且其 GWLP_USERDATA 保存的是本模块写入的 WindowBinding 指针（本窗口过程在 WM_NCCREATE 写入）。
unsafe fn enqueue_wnd_proc_panic(hwnd: *mut std::ffi::c_void, msg: u32) {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if ptr == 0 {
            return;
        }
        let binding = &*(ptr as *const WindowBinding);
        (&*binding.platform).enqueue_callback_failure(Error::new(
            Errc::PlatformError,
            format!("Windows wnd_proc ABI callback panicked while handling message 0x{msg:04x}"),
        ));
    }
}

/// 窗口过程正文：消息分发与状态更新。
///
/// # Safety
/// 调用者（wnd_proc 或本模块边界）必须保证 hwnd 存活、消息参数有效，且 GWLP_USERDATA 保存的 WindowBinding 指针在调用期间未被销毁。
unsafe fn wnd_proc_inner(
    hwnd: *mut std::ffi::c_void,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    // SAFETY: 调用者保证当前消息参数及 WindowBinding 有效；所有原始指针仅在这次同步消息处理期间访问。
    unsafe {
        if msg == WM_NCCREATE {
            let cs = lparam as *const CREATESTRUCTW;
            let this_ptr = (*cs).lpCreateParams;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, this_ptr as isize);
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if ptr == 0 {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let binding = &*(ptr as *const WindowBinding);
        let platform = &mut *binding.platform;
        if msg == WM_UIX_FRAME_OPPORTUNITY {
            if let Some(request) = complete_posted_frame(&binding.frame_pacer, wparam, lparam) {
                let window_id = binding.state.borrow().window_id;
                platform.push_event(
                    window_id,
                    UiEvent::frame_opportunity(request.token, Instant::now(), None),
                );
            }
            return 0;
        }
        // 还原事务结束后再重算非客户区，避免同步 FRAMECHANGED 重入旧最大化尺寸。
        if msg == WM_UIX_REFRESH_EXTENDED_FRAME {
            // 读取刷新时刻的样式，而不是还原 WM_SIZE 回调中的过渡样式。
            match super::custom_chrome::window_style(hwnd).and_then(|style| {
                // 当前消息已脱离还原回调，可以安全触发权威客户区的后续 WM_SIZE。
                super::custom_chrome::refresh_extended_client_frame(hwnd, style)
            }) {
                // 刷新成功后由同步产生的 WM_SIZE 继续走唯一尺寸事务。
                Ok(()) => {}
                // Win32 回调边界只记录错误，交由平台所有者统一处理。
                Err(error) => platform.enqueue_callback_failure(error),
            }
            // 自定义消息已完整处理，不交给默认窗口过程。
            return 0;
        }
        if msg == WM_DESTROY {
            clear_pending_frame(&binding.frame_pacer);
        }
        platform.handle_message(hwnd, &binding.state, &binding.ime, msg, wparam, lparam)
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/native/backends/windows/wnd_proc__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;

// ════════════════════════════════════════════════════════════════════════════
// 消息处理
// ════════════════════════════════════════════════════════════════════════════

impl WindowsPlatform {
    pub(crate) fn push_event(&mut self, window_id: crate::core::WindowId, event: UiEvent) {
        let event = event.for_window(window_id);
        self.lock_event_queue().push_back(event);
    }

    /// 处理窗口消息（由 wnd_proc 回调转发至此）。
    pub(crate) fn handle_message(
        &mut self,
        hwnd: *mut std::ffi::c_void,
        window: &std::rc::Rc<std::cell::RefCell<crate::native::windowing::shared::WindowState>>,
        ime: &std::cell::RefCell<WindowsImeState>,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        let window_id = window.borrow().window_id;
        match msg {
            WM_ACTIVATE => {
                // Microsoft 的自定义 DWM 帧契约要求在激活消息重新提交扩展边距；
                // 否则启动期设置可能被首次激活覆盖，导致无边框窗口没有系统阴影。
                let (state_maximized, state_minimized) = {
                    let state = window.borrow();
                    (state.maximized, state.minimized)
                };
                // 最小化期间 DWM 正在拥有非客户区状态转换；保留最后一个可见帧配置。
                if !state_minimized {
                    match super::custom_chrome::window_style(hwnd).and_then(|style| {
                        let maximized = super::custom_chrome::is_effectively_maximized(
                            hwnd,
                            style,
                            state_maximized,
                        );
                        super::custom_chrome::apply_dwm_frame_effects(hwnd, style, maximized)
                    }) {
                        Ok(()) => {}
                        Err(error) => self.enqueue_callback_failure(error),
                    }
                }
                self.def_window_proc(hwnd, msg, wparam, lparam)
            }
            WM_NCCALCSIZE => {
                let state_maximized = window.borrow().maximized;
                let style = match super::custom_chrome::window_style(hwnd) {
                    Ok(style) => style,
                    Err(error) => {
                        self.enqueue_callback_failure(error);
                        return self.def_window_proc(hwnd, msg, wparam, lparam);
                    }
                };
                let maximized =
                    super::custom_chrome::is_effectively_maximized(hwnd, style, state_maximized);
                // SAFETY: hwnd 属于当前同步消息的窗口；custom_chrome 只在本消息期间使用其句柄。
                if let Some(result) = unsafe {
                    super::custom_chrome::handle_nc_calc_size(
                        hwnd, style, wparam, lparam, maximized,
                    )
                } {
                    return result;
                }
                self.def_window_proc(hwnd, msg, wparam, lparam)
            }
            WM_NCHITTEST => {
                let resizable = window.borrow().resizable;
                let style = match super::custom_chrome::window_style(hwnd) {
                    Ok(style) => style,
                    Err(error) => {
                        self.enqueue_callback_failure(error);
                        return self.def_window_proc(hwnd, msg, wparam, lparam);
                    }
                };
                // SAFETY: hwnd 属于当前同步消息的窗口；custom_chrome 只在本消息期间使用其句柄。
                match unsafe {
                    super::custom_chrome::handle_nc_hit_test(hwnd, style, lparam, resizable)
                } {
                    Ok(Some(result)) => return result,
                    Ok(None) => {}
                    Err(error) => {
                        self.enqueue_callback_failure(error);
                    }
                }
                self.def_window_proc(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                // 先交给 app 关闭 engine/GL 资源；PlatformWindow::close 再销毁 HWND。
                self.push_event(window_id, UiEvent::close());
                0
            }
            WM_DESTROY => {
                self.forget_window(window_id);
                0
            }
            WM_SETTINGCHANGE | WM_THEMECHANGED => {
                let is_dark = WindowsDisplay::detect_os_theme().unwrap_or(false);
                self.route_system_theme_change(window_id, is_dark);
                self.def_window_proc(hwnd, msg, wparam, lparam)
            }
            WM_GETMINMAXINFO => {
                let default_result = self.def_window_proc(hwnd, msg, wparam, lparam);
                let (minimum, maximum) = {
                    let state = window.borrow();
                    (state.minimum_size, state.maximum_size)
                };
                if (minimum.is_some() || maximum.is_some()) && lparam != 0 {
                    // SAFETY: Win32 在当前同步消息期间提供唯一可写的 MINMAXINFO 指针。
                    let info = unsafe { &mut *(lparam as *mut MINMAXINFO) };
                    if let Err(error) = apply_window_track_constraints(hwnd, minimum, maximum, info)
                    {
                        self.enqueue_callback_failure(error);
                    }
                }
                default_result
            }
            WM_SIZE => {
                let dpi = super::dpi::dpi_for_window(hwnd);
                let w = super::dpi::physical_extent_to_logical(Self::loword(lparam) as i32, dpi);
                let h = super::dpi::physical_extent_to_logical(Self::hiword(lparam) as i32, dpi);
                // 最小化的 lParam 为零尺寸过渡值，不能驱动 DWM 或客户区几何事务。
                let chrome_style = if wparam == SIZE_MINIMIZED {
                    None
                } else {
                    match super::custom_chrome::window_style(hwnd) {
                        Ok(style) => Some(style),
                        Err(error) => {
                            self.enqueue_callback_failure(error);
                            None
                        }
                    }
                };
                #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                enum SizeAction {
                    Minimized,
                    Maximized,
                    Restored,
                    Resized,
                }
                let actions = {
                    let mut state = window.borrow_mut();
                    let mut acts = Vec::new();
                    let mut schedule_extended_frame_refresh = false;
                    match wparam {
                        SIZE_MINIMIZED => {
                            // Win32 在最小化时报告 0x0；保留最后一个有效客户区供恢复前查询。
                            state.minimized = true;
                            acts.push(SizeAction::Minimized);
                        }
                        SIZE_MAXIMIZED => {
                            state.minimized = false;
                            state.maximized = true;
                            acts.push(SizeAction::Maximized);
                            if w > 0 && h > 0 {
                                state.width = w;
                                state.height = h;
                                acts.push(SizeAction::Resized);
                            }
                        }
                        SIZE_RESTORED => {
                            let was_min = state.minimized;
                            let was_max = state.maximized;
                            state.minimized = false;
                            state.maximized = false;
                            if was_min || was_max {
                                acts.push(SizeAction::Restored);
                            }
                            // 最大化期 NCCALCSIZE 内缩边框；还原后需 FRAMECHANGED
                            // 才能把客户区重新扩到外窗，否则四周透出桌面。
                            schedule_extended_frame_refresh = was_max;
                            if w > 0 && h > 0 {
                                state.width = w;
                                state.height = h;
                                acts.push(SizeAction::Resized);
                            }
                        }
                        _ => {
                            if w > 0 && h > 0 {
                                state.width = w;
                                state.height = h;
                                acts.push(SizeAction::Resized);
                            }
                        }
                    }
                    drop(state);
                    if let Some(style) = chrome_style {
                        let mut chrome_failure = None;
                        if schedule_extended_frame_refresh {
                            // 延迟到当前还原 WM_SIZE 返回后再重算非客户区，避免旧尺寸重入。
                            // SAFETY: hwnd 属于当前同步消息的窗口；PostMessageW 投递自定义消息不持有指针。
                            if unsafe { PostMessageW(hwnd, WM_UIX_REFRESH_EXTENDED_FRAME, 0, 0) }
                                == 0
                            {
                                // 统一使用平台错误通道报告消息调度失败。
                                let error = super::util::windows_diag(
                                    Errc::PlatformError,
                                    "custom chrome: PostMessageW(refresh extended frame) failed",
                                );
                                chrome_failure = Some(error);
                            }
                        }
                        let maximized = matches!(wparam, SIZE_MAXIMIZED)
                            || super::custom_chrome::is_effectively_maximized(hwnd, style, false);
                        if let Err(error) =
                            super::custom_chrome::apply_dwm_frame_effects(hwnd, style, maximized)
                        {
                            if chrome_failure.is_none() {
                                chrome_failure = Some(error);
                            }
                        }
                        if let Some(error) = chrome_failure {
                            self.enqueue_callback_failure(error);
                        }
                    }
                    acts
                };
                for action in actions {
                    match action {
                        SizeAction::Minimized => {
                            self.push_event(
                                window_id,
                                UiEvent {
                                    window_id: None,
                                    type_: UiEventType::WindowMinimize,
                                    payload: UiEventPayload::None,
                                },
                            );
                        }
                        SizeAction::Maximized => {
                            self.push_event(
                                window_id,
                                UiEvent {
                                    window_id: None,
                                    type_: UiEventType::WindowMaximize,
                                    payload: UiEventPayload::None,
                                },
                            );
                        }
                        SizeAction::Restored => {
                            self.push_event(
                                window_id,
                                UiEvent {
                                    window_id: None,
                                    type_: UiEventType::WindowRestore,
                                    payload: UiEventPayload::None,
                                },
                            );
                        }
                        SizeAction::Resized => {
                            self.push_event(window_id, UiEvent::resize(w, h));
                        }
                    }
                }
                0
            }
            WM_DPICHANGED => {
                // Windows 在 lParam 中给出按新 DPI 计算的 physical 外窗矩形；
                // SetWindowPos 同步产生的 WM_SIZE 继续走唯一 resize/graphics 重建路径。
                // SAFETY: hwnd 存活（当前同步消息）；lparam 指向 Win32 提供的 RECT，调用期间有效。
                let result = unsafe {
                    super::dpi::apply_suggested_window_rect(
                        hwnd,
                        lparam as *const super::bindings::RECT,
                    )
                };
                if let Err(error) = result {
                    self.enqueue_callback_failure(error);
                }
                0
            }
            WM_SHOWWINDOW => {
                let visible = wparam != 0;
                window.borrow_mut().visible = visible;
                self.push_event(
                    window_id,
                    if visible {
                        UiEvent::window_show()
                    } else {
                        UiEvent::window_hide()
                    },
                );
                0
            }
            WM_MOVE => {
                let mut rect = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                // SAFETY: hwnd 属于当前同步窗口消息；rect 在调用期间有效可写。
                if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
                    self.enqueue_callback_failure(super::util::windows_diag(
                        crate::native::Errc::PlatformError,
                        "WM_MOVE GetWindowRect failed",
                    ));
                } else {
                    let mut state = window.borrow_mut();
                    state.pos_x = rect.left;
                    state.pos_y = rect.top;
                }
                0
            }
            WM_SETFOCUS => {
                self.push_event(
                    window_id,
                    UiEvent {
                        window_id: None,
                        type_: UiEventType::WindowFocus,
                        payload: UiEventPayload::None,
                    },
                );
                0
            }
            WM_KILLFOCUS => {
                let mut ime = ime.borrow_mut();
                if let Some(event) = ime_end_composition_event(&mut ime) {
                    self.push_event(window_id, event);
                }
                ime.reset_text_decoder();
                self.push_event(
                    window_id,
                    UiEvent {
                        window_id: None,
                        type_: UiEventType::WindowBlur,
                        payload: UiEventPayload::None,
                    },
                );
                0
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let key = Self::vk_to_keycode(wparam as u32);
                let mods = Self::get_modifier_state();
                self.push_event(window_id, UiEvent::key_down(key, mods));
                // 系统组合键仍须交由 DefWindowProc 处理：例如 Alt+F4 会生成
                // WM_SYSCOMMAND/WM_CLOSE，随后再经上面的安全 teardown 路径关闭窗口。
                if msg == WM_SYSKEYDOWN {
                    self.def_window_proc(hwnd, msg, wparam, lparam)
                } else {
                    0
                }
            }
            WM_KEYUP | WM_SYSKEYUP => {
                let key = Self::vk_to_keycode(wparam as u32);
                let mods = Self::get_modifier_state();
                self.push_event(window_id, UiEvent::key_up(key, mods));
                if msg == WM_SYSKEYUP {
                    self.def_window_proc(hwnd, msg, wparam, lparam)
                } else {
                    0
                }
            }
            WM_CHAR => {
                if let Some(text) = ime.borrow_mut().decode_utf16_unit(wparam as u16) {
                    self.push_event(window_id, UiEvent::text_input(text));
                }
                0
            }
            WM_IME_STARTCOMPOSITION => {
                if self.text_input_subsys.tsf_session_active_for(window_id) {
                    return 0;
                }
                if let Some(event) = ime_start_composition_event(&mut ime.borrow_mut()) {
                    self.push_event(window_id, event);
                }
                0
            }
            WM_IME_COMPOSITION => {
                // TSF TextStore 已接管时跳过 IMM32，避免双发。
                if self.text_input_subsys.tsf_session_active_for(window_id) {
                    return 0;
                }
                let flags = lparam as u32;
                let result_read = if flags & GCS_RESULTSTR != 0 {
                    match result_string(hwnd) {
                        Ok(value) => ImmStringRead::from_flagged_result(true, Ok(value)),
                        Err(err) => {
                            self.enqueue_callback_failure(err);
                            ImmStringRead::Skipped
                        }
                    }
                } else {
                    ImmStringRead::Skipped
                };
                let comp_read = if flags & GCS_COMPSTR != 0 {
                    match composition_string(hwnd) {
                        Ok(value) => ImmStringRead::from_flagged_result(true, Ok(value)),
                        Err(err) => {
                            self.enqueue_callback_failure(err);
                            ImmStringRead::Skipped
                        }
                    }
                } else {
                    ImmStringRead::Skipped
                };

                let (handled, events) =
                    ime_composition_events(&mut ime.borrow_mut(), result_read, comp_read);
                for event in events {
                    self.push_event(window_id, event);
                }
                if handled {
                    0
                } else {
                    self.def_window_proc(hwnd, msg, wparam, lparam)
                }
            }
            WM_IME_ENDCOMPOSITION => {
                if self.text_input_subsys.tsf_session_active_for(window_id) {
                    return 0;
                }
                if let Some(event) = ime_end_composition_event(&mut ime.borrow_mut()) {
                    self.push_event(window_id, event);
                }
                0
            }
            WM_LBUTTONDOWN => {
                self.handle_mouse_down(hwnd, window_id, lparam, MouseButton::Left);
                0
            }
            WM_LBUTTONDBLCLK => {
                self.handle_mouse_double_click(hwnd, window_id, lparam, MouseButton::Left);
                0
            }
            WM_LBUTTONUP => {
                self.handle_mouse_up(hwnd, window_id, lparam, MouseButton::Left);
                0
            }
            WM_RBUTTONDOWN => {
                self.handle_mouse_down(hwnd, window_id, lparam, MouseButton::Right);
                0
            }
            WM_RBUTTONDBLCLK => {
                self.handle_mouse_double_click(hwnd, window_id, lparam, MouseButton::Right);
                0
            }
            WM_RBUTTONUP => {
                self.handle_mouse_up(hwnd, window_id, lparam, MouseButton::Right);
                0
            }
            WM_MBUTTONDOWN => {
                self.handle_mouse_down(hwnd, window_id, lparam, MouseButton::Middle);
                0
            }
            WM_MBUTTONDBLCLK => {
                self.handle_mouse_double_click(hwnd, window_id, lparam, MouseButton::Middle);
                0
            }
            WM_MBUTTONUP => {
                self.handle_mouse_up(hwnd, window_id, lparam, MouseButton::Middle);
                0
            }
            WM_MOUSEMOVE => {
                let pos = self.mouse_pos_from_lparam(hwnd, lparam);
                self.push_event(window_id, UiEvent::pointer_move(pos));
                0
            }
            WM_MOUSEWHEEL => {
                let screen_pt = POINT {
                    x: Self::loword_signed(lparam) as i32,
                    y: Self::hiword_signed(lparam) as i32,
                };
                let mut client_pt = screen_pt;
                // SAFETY: hwnd 属于当前同步消息的窗口；client_pt 为栈上可写坐标，ScreenToClient 同步转换。
                unsafe {
                    ScreenToClient(hwnd, &mut client_pt);
                }
                let dpi = super::dpi::dpi_for_window(hwnd);
                let pos = Point::new(
                    super::dpi::physical_point_to_logical(client_pt.x, dpi),
                    super::dpi::physical_point_to_logical(client_pt.y, dpi),
                );
                let delta = (Self::hiword_usize(wparam) as i16) as i32;
                let delta_y = -(delta as f32) / 120.0;
                let mods = Self::get_modifier_state();
                self.push_event(window_id, UiEvent::wheel(pos, 0.0, delta_y, mods));
                0
            }
            WM_TIMER => {
                let timer_id = wparam as u32;
                let mut set = self
                    .single_shot_timers
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                if set.remove(&timer_id) {
                    // SAFETY: hwnd 属于当前同步消息的窗口；timer_id 为本平台创建并登记的计时器。
                    unsafe {
                        KillTimer(hwnd, timer_id);
                    }
                }
                drop(set);
                self.push_event(window_id, UiEvent::timer(timer_id));
                0
            }
            WM_DROPFILES => {
                let hdrop = lparam as *mut std::ffi::c_void;
                self.handle_file_drop(hwnd, window_id, hdrop);
                0
            }
            WM_ERASEBKGND => {
                // 阻止系统擦除客户区，避免 resize 拖拽时出现黑色闪屏。
                1
            }
            WM_SETCURSOR => {
                let hit = Self::loword(lparam) as u32;
                if hit == HTCLIENT {
                    // SAFETY: LoadCursorW 的 null 模块句柄表示系统游标，IDC_ARROW 为存活字符串常量；SetCursor 接受系统游标句柄。
                    unsafe {
                        let cursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW as *const u16);
                        SetCursor(cursor);
                    }
                    return 1;
                }
                self.def_window_proc(hwnd, msg, wparam, lparam)
            }
            _ => self.def_window_proc(hwnd, msg, wparam, lparam),
        }
    }
}
