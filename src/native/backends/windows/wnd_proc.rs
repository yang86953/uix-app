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
use super::ffi::*;
use super::frame_pacer::{clear_pending_frame, complete_posted_frame};
use super::ime_dispatch::{
    ime_composition_events, ime_end_composition_event, ime_start_composition_event, ImmStringRead,
};
use super::platform::{WindowBinding, WindowsPlatform};
use super::text_input::{composition_string, result_string, WindowsImeState};

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
        if let Ok(mut queue) = self.event_queue.lock() {
            queue.push_back(event);
        }
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
            WM_CLOSE => {
                // 先交给 app 关闭 engine/GL 资源；PlatformWindow::close 再销毁 HWND。
                self.push_event(window_id, UiEvent::close());
                0
            }
            WM_DESTROY => {
                self.forget_window(window_id);
                0
            }
            WM_SIZE => {
                let w = Self::loword(lparam) as i32;
                let h = Self::hiword(lparam) as i32;
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
                            acts.push(SizeAction::Resized);
                        }
                        _ => {
                            acts.push(SizeAction::Resized);
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
                let mut state = window.borrow_mut();
                state.pos_x = Self::loword_signed(lparam) as i32;
                state.pos_y = Self::hiword_signed(lparam) as i32;
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
                            crate::core::log::error_fn(err.short_what());
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
                            crate::core::log::error_fn(err.short_what());
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
            WM_LBUTTONUP => {
                self.handle_mouse_up(window_id, lparam, MouseButton::Left);
                0
            }
            WM_RBUTTONDOWN => {
                self.handle_mouse_down(hwnd, window_id, lparam, MouseButton::Right);
                0
            }
            WM_RBUTTONUP => {
                self.handle_mouse_up(window_id, lparam, MouseButton::Right);
                0
            }
            WM_MBUTTONDOWN => {
                self.handle_mouse_down(hwnd, window_id, lparam, MouseButton::Middle);
                0
            }
            WM_MBUTTONUP => {
                self.handle_mouse_up(window_id, lparam, MouseButton::Middle);
                0
            }
            WM_MOUSEMOVE => {
                let pos = self.mouse_pos_from_lparam(lparam);
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
                let pos = Point::new(client_pt.x as f32, client_pt.y as f32);
                let delta = (Self::hiword_usize(wparam) as i16) as i32;
                let delta_y = -(delta as f32) / 120.0;
                let mods = Self::get_modifier_state();
                self.push_event(window_id, UiEvent::wheel(pos, 0.0, delta_y, mods));
                0
            }
            WM_TIMER => {
                let timer_id = wparam as u32;
                if let Ok(mut set) = self.single_shot_timers.lock() {
                    if set.remove(&timer_id) {
                        unsafe {
                            KillTimer(hwnd, timer_id);
                        }
                    }
                }
                self.push_event(window_id, UiEvent::timer(timer_id));
                0
            }
            WM_DROPFILES => {
                let hdrop = lparam as *mut std::ffi::c_void;
                self.handle_file_drop(window_id, hdrop);
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
