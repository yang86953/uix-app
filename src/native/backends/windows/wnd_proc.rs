// ============================================================================
// platform/windows/wnd_proc.rs — Windows 窗口过程 + 消息处理
//
// wnd_proc: Win32 窗口过程回调（extern "system"）
// handle_message: 消息分发 → UiEvent 转换 → 状态更新
// ============================================================================

#![cfg(windows)]
#![allow(non_snake_case)]

use crate::core::Point;
use crate::native::traits::*;

use super::bindings::*;
use super::consts::*;
use super::ffi::*;
use super::platform::WindowsPlatform;

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
    let platform = &mut *(ptr as *mut WindowsPlatform);
    platform.handle_message(msg, wparam, lparam)
}

// ════════════════════════════════════════════════════════════════════════════
// 消息处理
// ════════════════════════════════════════════════════════════════════════════

impl WindowsPlatform {
    pub(crate) fn push_event(&mut self, event: UiEvent) {
        self.event_queue.push_back(event);
    }

    /// 处理窗口消息（由 wnd_proc 回调转发至此）。
    pub(crate) fn handle_message(&mut self, msg: u32, wparam: usize, lparam: isize) -> isize {
        match msg {
            WM_CLOSE => {
                self.push_event(UiEvent::close());
                unsafe {
                    DestroyWindow(self.hwnd);
                }
                self.hwnd = std::ptr::null_mut();
                0
            }
            WM_DESTROY => {
                unsafe {
                    PostQuitMessage(0);
                }
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
                    let mut state = self.window.borrow_mut();
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
                            self.push_event(UiEvent {
                                type_: UiEventType::WindowMinimize,
                                payload: UiEventPayload::None,
                            });
                        }
                        SizeAction::Maximized => {
                            self.push_event(UiEvent {
                                type_: UiEventType::WindowMaximize,
                                payload: UiEventPayload::None,
                            });
                        }
                        SizeAction::Restored => {
                            self.push_event(UiEvent {
                                type_: UiEventType::WindowRestore,
                                payload: UiEventPayload::None,
                            });
                        }
                        SizeAction::Resized => {
                            self.push_event(UiEvent::resize(w, h));
                        }
                    }
                }
                0
            }
            WM_MOVE => {
                let mut state = self.window.borrow_mut();
                state.pos_x = Self::loword(lparam) as i32;
                state.pos_y = Self::hiword(lparam) as i32;
                0
            }
            WM_SETFOCUS => {
                self.push_event(UiEvent {
                    type_: UiEventType::WindowFocus,
                    payload: UiEventPayload::None,
                });
                0
            }
            WM_KILLFOCUS => {
                self.push_event(UiEvent {
                    type_: UiEventType::WindowBlur,
                    payload: UiEventPayload::None,
                });
                0
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let key = Self::vk_to_keycode(wparam as u32);
                let mods = Self::get_modifier_state();
                self.push_event(UiEvent::key_down(key, mods));
                0
            }
            WM_KEYUP | WM_SYSKEYUP => {
                let key = Self::vk_to_keycode(wparam as u32);
                let mods = Self::get_modifier_state();
                self.push_event(UiEvent::key_up(key, mods));
                0
            }
            WM_CHAR => {
                if let Some(ch) = std::char::from_u32(wparam as u32) {
                    self.push_event(UiEvent::text_input(ch.to_string()));
                }
                0
            }
            WM_LBUTTONDOWN => {
                self.handle_mouse_down(lparam, MouseButton::Left);
                0
            }
            WM_LBUTTONUP => {
                self.handle_mouse_up(lparam, MouseButton::Left);
                0
            }
            WM_RBUTTONDOWN => {
                self.handle_mouse_down(lparam, MouseButton::Right);
                0
            }
            WM_RBUTTONUP => {
                self.handle_mouse_up(lparam, MouseButton::Right);
                0
            }
            WM_MBUTTONDOWN => {
                self.handle_mouse_down(lparam, MouseButton::Middle);
                0
            }
            WM_MBUTTONUP => {
                self.handle_mouse_up(lparam, MouseButton::Middle);
                0
            }
            WM_MOUSEMOVE => {
                let pos = self.mouse_pos_from_lparam(lparam);
                self.push_event(UiEvent::pointer_move(pos));
                0
            }
            WM_MOUSEWHEEL => {
                let screen_pt = POINT {
                    x: Self::loword(lparam) as i32,
                    y: Self::hiword(lparam) as i32,
                };
                let mut client_pt = screen_pt;
                unsafe {
                    ScreenToClient(self.hwnd, &mut client_pt);
                }
                let pos = Point::new(client_pt.x as f32, client_pt.y as f32);
                let delta = (Self::hiword_usize(wparam) as i16) as i32;
                let delta_y = -(delta as f32) / 120.0;
                let mods = Self::get_modifier_state();
                self.push_event(UiEvent::wheel(pos, 0.0, delta_y, mods));
                0
            }
            WM_TIMER => {
                let timer_id = wparam as u32;
                if let Ok(mut set) = self.single_shot_timers.lock() {
                    if set.remove(&timer_id) {
                        unsafe {
                            KillTimer(self.hwnd, timer_id);
                        }
                    }
                }
                self.push_event(UiEvent::timer(timer_id));
                0
            }
            WM_DROPFILES => {
                let hdrop = lparam as *mut std::ffi::c_void;
                self.handle_file_drop(hdrop);
                0
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
                self.def_window_proc(msg, wparam, lparam)
            }
            _ => self.def_window_proc(msg, wparam, lparam),
        }
    }
}
