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
use super::platform::{WindowBinding, WindowsPlatform};

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
    platform.handle_message(hwnd, &binding.state, msg, wparam, lparam)
}

// ════════════════════════════════════════════════════════════════════════════
// 消息处理
// ════════════════════════════════════════════════════════════════════════════

impl WindowsPlatform {
    pub(crate) fn push_event(&mut self, window_id: crate::core::WindowId, event: UiEvent) {
        let event = event.for_window(window_id);
        self.event_queue.push_back(event);
    }

    /// 处理窗口消息（由 wnd_proc 回调转发至此）。
    pub(crate) fn handle_message(
        &mut self,
        hwnd: *mut std::ffi::c_void,
        window: &std::rc::Rc<std::cell::RefCell<crate::native::shared::WindowState>>,
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
            WM_MOVE => {
                let mut state = window.borrow_mut();
                state.pos_x = Self::loword(lparam) as i32;
                state.pos_y = Self::hiword(lparam) as i32;
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
                0
            }
            WM_KEYUP | WM_SYSKEYUP => {
                let key = Self::vk_to_keycode(wparam as u32);
                let mods = Self::get_modifier_state();
                self.push_event(window_id, UiEvent::key_up(key, mods));
                0
            }
            WM_CHAR => {
                if let Some(ch) = std::char::from_u32(wparam as u32) {
                    self.push_event(window_id, UiEvent::text_input(ch.to_string()));
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
                    x: Self::loword(lparam) as i32,
                    y: Self::hiword(lparam) as i32,
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
