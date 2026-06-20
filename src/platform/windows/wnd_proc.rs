// ============================================================================
// uix-platform/src/windows/wnd_proc.rs — 窗口过程回调 & 消息分发
// ============================================================================
//
// # Safety
//
// 窗口过程 `wnd_proc` 通过 `GWLP_USERDATA` 存储的 `*mut WindowsPlatform` 指针
// 重建 `&mut WindowsPlatform` 引用。安全前提：
//   1. WindowsPlatform 必须在创建窗口的同一线程上存活
//   2. `WM_NCCREATE` 中必须写入有效指针
//   3. 窗口过程运行时不能有其他 `&mut` 引用共存
//   4. WindowsPlatform 的 Drop 必须在窗口销毁后执行
//
// ============================================================================

use super::bindings::{CREATESTRUCTW, POINT};
use super::consts::*;
use super::ffi::*;
use super::platform::WindowsPlatform;
use crate::base::*;
use crate::platform::event::*;

use std::ptr;

impl WindowsPlatform {
    /// 窗口过程消息处理 — 由 `wnd_proc` 回调转发至此。
    pub(crate) fn handle_message(&mut self, msg: u32, wparam: usize, lparam: isize) -> isize {
        match msg {
            WM_CLOSE => {
                self.push_event(UiEvent::close());
                unsafe {
                    DestroyWindow(self.hwnd);
                }
                self.hwnd = ptr::null_mut();
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
                self.width = w;
                self.height = h;

                // 同步呈现器尺寸
                if let Err(e) = self.presenter.resize(w, h) {
                    log::debug!(
                        "WindowsPlatform: presenter resize failed: {}",
                        e.short_what()
                    );
                }

                match wparam {
                    SIZE_MINIMIZED => {
                        self.minimized = true;
                        self.push_event(UiEvent {
                            type_: UiEventType::WindowMinimize,
                            payload: UiEventPayload::None,
                        });
                    }
                    SIZE_MAXIMIZED => {
                        self.minimized = false;
                        self.maximized = true;
                        self.push_event(UiEvent {
                            type_: UiEventType::WindowMaximize,
                            payload: UiEventPayload::None,
                        });
                        // 最大化时窗口尺寸已变，必须推送 resize 事件
                        // 否则引擎和 widget 树不知道新尺寸，内部组件无法响应式变化
                        self.push_event(UiEvent::resize(w, h));
                    }
                    SIZE_RESTORED => {
                        let was_min = self.minimized;
                        let was_max = self.maximized;
                        self.minimized = false;
                        self.maximized = false;
                        if was_min || was_max {
                            self.push_event(UiEvent {
                                type_: UiEventType::WindowRestore,
                                payload: UiEventPayload::None,
                            });
                        }
                        self.push_event(UiEvent::resize(w, h));
                    }
                    _ => {
                        self.push_event(UiEvent::resize(w, h));
                    }
                }
                0
            }

            WM_MOVE => {
                self.pos_x = Self::loword(lparam) as i32;
                self.pos_y = Self::hiword(lparam) as i32;
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

            // SAFETY: WM_CHAR delivers valid Unicode code points (UTF-16).
            // Use char::from_u32 to safely handle the full Unicode range,
            // avoiding truncation to u8 and potential invalid char values.
            WM_CHAR => {
                if let Some(ch) = std::char::from_u32(wparam as u32) {
                    self.push_event(UiEvent::key_press(ch.to_string()));
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
                self.push_event(UiEvent::mouse_move(pos));
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
                // 取反：Windows 正 delta = 滚轮向上，框架约定正值为向下滚动
                let delta_y = -(delta as f32) / 120.0;
                let mods = Self::get_modifier_state();
                self.push_event(UiEvent::mouse_wheel(pos, 0.0, delta_y, mods));
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
                        let cursor = LoadCursorW(ptr::null_mut(), IDC_ARROW as *const u16);
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

// ════════════════════════════════════════════════════════════════════════════
// 窗口过程（extern "system" 回调）
//
// # Safety
//
// 1. `WM_NCCREATE` 写入 `GWLP_USERDATA` 存储 `*mut WindowsPlatform`
// 2. 该指针在窗口销毁前有效
// 3. 窗口过程在 WindowsPlatform 所属线程上调用
// 4. 调用期间没有其他 `&mut WindowsPlatform` 引用
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

    // 从 GWLP_USERDATA 重建 &mut WindowsPlatform
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let platform = &mut *(ptr as *mut WindowsPlatform);
    platform.handle_message(msg, wparam, lparam)
}
