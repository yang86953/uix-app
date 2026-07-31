// ============================================================================
// platform/windows/wnd_proc.rs — Windows 窗口过程 + 消息处理
//
// wnd_proc: Win32 窗口过程回调（extern "system"）
// handle_message: 消息分发 → UiEvent 转换 → 状态更新
// ============================================================================

#![cfg(windows)]
#![allow(non_snake_case)]

use std::time::Instant;

use crate::core::Point;
use crate::native::traits::*;

use super::bindings::*;
use super::consts::*;
use super::display::WindowsDisplay;
use super::ffi::*;
use super::frame_pacer::{clear_pending_frame, complete_posted_frame};
use super::ime_dispatch::{
    ime_composition_events, ime_end_composition_event, ime_start_composition_event, ImmStringRead,
};
use super::platform::{WindowBinding, WindowsPlatform};
use super::text_input::{composition_string, result_string, WindowsImeState};

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

pub(crate) unsafe extern "system" fn wnd_proc(
    hwnd: *mut std::ffi::c_void,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
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
    if msg == WM_DESTROY {
        clear_pending_frame(&binding.frame_pacer);
    }
    platform.handle_message(hwnd, &binding.state, &binding.ime, msg, wparam, lparam)
}

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
        window: &std::rc::Rc<std::cell::RefCell<crate::native::shared::WindowState>>,
        ime: &std::cell::RefCell<WindowsImeState>,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        let window_id = window.borrow().window_id;
        match msg {
            WM_NCCALCSIZE => {
                let state_maximized = window.borrow().maximized;
                let maximized =
                    super::custom_chrome::is_effectively_maximized(hwnd, state_maximized);
                if let Some(result) = unsafe {
                    super::custom_chrome::handle_nc_calc_size(hwnd, wparam, lparam, maximized)
                } {
                    return result;
                }
                self.def_window_proc(hwnd, msg, wparam, lparam)
            }
            WM_NCHITTEST => {
                let resizable = window.borrow().resizable;
                if let Some(result) =
                    unsafe { super::custom_chrome::handle_nc_hit_test(hwnd, lparam, resizable) }
                {
                    return result;
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
                self.route_system_theme_change(window_id, WindowsDisplay::detect_os_theme());
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
                #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                enum SizeAction {
                    Minimized,
                    Maximized,
                    Restored,
                    Resized,
                }
                let actions = {
                    let mut state = window.borrow_mut();
                    state.width = w;
                    state.height = h;
                    let mut acts = Vec::new();
                    let mut refresh_extended_frame = false;
                    match wparam {
                        SIZE_MINIMIZED => {
                            state.minimized = true;
                            acts.push(SizeAction::Minimized);
                        }
                        SIZE_MAXIMIZED => {
                            state.minimized = false;
                            state.maximized = true;
                            acts.push(SizeAction::Maximized);
                            acts.push(SizeAction::Resized);
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
                            refresh_extended_frame = was_max;
                            acts.push(SizeAction::Resized);
                        }
                        _ => {
                            acts.push(SizeAction::Resized);
                        }
                    }
                    drop(state);
                    if refresh_extended_frame {
                        super::custom_chrome::refresh_extended_client_frame(hwnd);
                    }
                    let maximized = matches!(wparam, SIZE_MAXIMIZED)
                        || super::custom_chrome::is_effectively_maximized(hwnd, false);
                    super::custom_chrome::apply_dwm_frame_effects(hwnd, maximized);
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
