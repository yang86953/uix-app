// ============================================================================
// platform/linux/wayland/creation.rs — Seat 绑定与输入设备初始化
//
// 窗口创建（surface/toplevel/decoration 等）已迁移到 window_impl.rs。
// 此文件仅保留 WaylandBackend 的 seat 绑定（指针+键盘+剪贴板），
// 该绑定只需执行一次，多窗口共享。
// ============================================================================

use std::collections::VecDeque;
use std::os::fd::IntoRawFd;
use std::sync::{Arc, Mutex};

use wayland_client::protocol::{wl_data_device, wl_keyboard, wl_pointer, wl_seat};

use super::{HeldKeyInfo, WaylandBackend};
use crate::core::{Point, WindowId};
use crate::native::backends::linux::wayland::keycode::{keycode_to_char, linux_keycode_to_keycode};
use crate::native::windowing::event::*;
use crate::native::windowing::input::{KeyMod, MouseButton};

fn enqueue_for_window(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    window_id: Option<WindowId>,
    event: UiEvent,
) {
    let Some(window_id) = window_id else {
        return;
    };
    events
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push_back(event.for_window(window_id));
}

impl WaylandBackend {
    /// 确保 seat 已绑定（指针 + 键盘 + 剪贴板数据设备）。
    /// 多窗口共享同一份输入设备绑定，仅首次调用时执行实际绑定。
    pub(crate) fn ensure_seat_and_input(&mut self) {
        if self.seat.is_some() {
            return; // 已绑定
        }

        let seat = match self._globals.instantiate_exact::<wl_seat::WlSeat>(5) {
            Ok(s) => s,
            Err(_) => {
                tracing::warn!("Wayland: no wl_seat available, input unavailable");
                return;
            }
        };

        // ── 剪贴板数据设备 ──────────────────────────────────
        if let Some(ref sdm) = self.data_device_manager {
            let dev = sdm.get_data_device(&seat);
            let clipboard_read = self.clipboard_read.clone();
            let clipboard_text = self.clipboard_text.clone();
            let owns_clipboard = self.owns_clipboard.clone();
            dev.quick_assign(move |_, event, _| {
                if let wl_data_device::Event::Selection { id } = event {
                    *owns_clipboard
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) = false;
                    if let Some(offer) = id {
                        match super::clipboard::ClipboardRead::create_pipe() {
                            Ok((read, write_fd)) => {
                                offer.receive(
                                    "text/plain;charset=utf-8".to_string(),
                                    write_fd.into_raw_fd(),
                                );
                                *clipboard_read
                                    .lock()
                                    .unwrap_or_else(|error| error.into_inner()) = Some(read);
                            }
                            Err(error) => {
                                tracing::error!("Wayland clipboard pipe creation failed: {error}")
                            }
                        }
                    } else {
                        *clipboard_read
                            .lock()
                            .unwrap_or_else(|error| error.into_inner()) = None;
                        clipboard_text
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .clear();
                    }
                }
            });
            self.data_device = Some(dev);
        }

        // ── 指针 + 键盘 ─────────────────────────────────────
        let ptr_events = self.events.clone();
        let ptr_pos = self.last_pointer.clone();
        let keys_down = self.keys_down.clone();
        let wl_pointer_handle = self.pointer.clone();
        let wl_keyboard_handle = self.keyboard.clone();
        // 按键重复（客户端侧实现）
        let repeat_rate = self.repeat_rate.clone();
        let repeat_delay = self.repeat_delay.clone();
        let held_key_info = self.held_key_info.clone();
        let last_repeat_time = self.last_repeat_time.clone();
        let pointer_serial = self.last_input_serial.clone();
        let keyboard_serial = self.last_input_serial.clone();
        let surface_windows = self.surface_windows.clone();
        let _kbd = seat.get_keyboard();

        seat.quick_assign(move |seat, event, _| {
            if let wl_seat::Event::Capabilities { capabilities } = event {
                use wayland_client::protocol::wl_seat::Capability;
                if capabilities.contains(Capability::Pointer) {
                    let ev = ptr_events.clone();
                    let pos = ptr_pos.clone();
                    let pointer_serial = pointer_serial.clone();
                    let targets = surface_windows.clone();
                    let ptr = seat.get_pointer();
                    ptr.quick_assign(move |_, event, _| match event {
                        wl_pointer::Event::Enter {
                            surface,
                            surface_x,
                            surface_y,
                            ..
                        } => {
                            let window_id = targets
                                .lock()
                                .unwrap_or_else(|error| error.into_inner())
                                .pointer_enter(surface.as_ref().id());
                            if window_id.is_none() {
                                return;
                            }
                            let p = Point::new(surface_x as f32, surface_y as f32);
                            if let Ok(mut lp) = pos.lock() {
                                lp.position = p;
                            }
                            enqueue_for_window(&ev, window_id, UiEvent::pointer_move(p));
                        }
                        wl_pointer::Event::Motion {
                            surface_x,
                            surface_y,
                            ..
                        } => {
                            let window_id = targets
                                .lock()
                                .unwrap_or_else(|error| error.into_inner())
                                .pointer_target();
                            if window_id.is_none() {
                                return;
                            }
                            let p = Point::new(surface_x as f32, surface_y as f32);
                            if let Ok(mut lp) = pos.lock() {
                                lp.position = p;
                            }
                            enqueue_for_window(&ev, window_id, UiEvent::pointer_move(p));
                        }
                        wl_pointer::Event::Leave { surface, .. } => {
                            targets
                                .lock()
                                .unwrap_or_else(|error| error.into_inner())
                                .pointer_leave(surface.as_ref().id());
                        }
                        wl_pointer::Event::Button {
                            serial,
                            button,
                            state,
                            ..
                        } => {
                            let window_id = targets
                                .lock()
                                .unwrap_or_else(|error| error.into_inner())
                                .pointer_target();
                            if window_id.is_none() {
                                return;
                            }
                            let btn = match button {
                                0x110 => MouseButton::Left,
                                0x111 => MouseButton::Right,
                                0x112 => MouseButton::Middle,
                                _ => MouseButton::None,
                            };
                            let click_pos = pos.lock().map(|lp| lp.position).unwrap_or_default();
                            if state == wl_pointer::ButtonState::Pressed {
                                pointer_serial
                                    .lock()
                                    .unwrap_or_else(|error| error.into_inner())
                                    .record(serial);
                                enqueue_for_window(
                                    &ev,
                                    window_id,
                                    UiEvent::pointer_down(click_pos, btn),
                                );
                            } else {
                                enqueue_for_window(
                                    &ev,
                                    window_id,
                                    UiEvent::pointer_up(click_pos, btn),
                                );
                            }
                        }
                        wl_pointer::Event::Axis { axis, value, .. } => {
                            let window_id = targets
                                .lock()
                                .unwrap_or_else(|error| error.into_inner())
                                .pointer_target();
                            if window_id.is_none() {
                                return;
                            }
                            let (dx, dy) = match axis {
                                wl_pointer::Axis::VerticalScroll => (0.0, value),
                                wl_pointer::Axis::HorizontalScroll => (value, 0.0),
                                _ => (0.0, 0.0),
                            };
                            if dx != 0.0 || dy != 0.0 {
                                let scroll_pos =
                                    pos.lock().map(|lp| lp.position).unwrap_or_default();
                                enqueue_for_window(
                                    &ev,
                                    window_id,
                                    UiEvent::wheel(scroll_pos, dx as f32, dy as f32, KeyMod::NONE),
                                );
                            }
                        }
                        _ => {}
                    });
                    if let Ok(mut p) = wl_pointer_handle.lock() {
                        *p = Some(ptr);
                    }
                } else {
                    surface_windows
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .clear_pointer_focus();
                    if let Ok(mut p) = wl_pointer_handle.lock() {
                        *p = None;
                    }
                }
                if capabilities.contains(Capability::Keyboard) {
                    let ev = ptr_events.clone();
                    let kd = keys_down.clone();
                    let mods = Arc::new(Mutex::new(KeyMod::NONE));
                    let repeat_rate = repeat_rate.clone();
                    let repeat_delay = repeat_delay.clone();
                    let held_key_info = held_key_info.clone();
                    let last_repeat_time = last_repeat_time.clone();
                    let keyboard_serial = keyboard_serial.clone();
                    let targets = surface_windows.clone();
                    let kbd = seat.get_keyboard();
                    kbd.quick_assign(move |_, event, _| {
                        match event {
                            wl_keyboard::Event::Enter { surface, .. } => {
                                let (previous_window, window_id) = {
                                    let mut targets =
                                        targets.lock().unwrap_or_else(|error| error.into_inner());
                                    let previous = targets.keyboard_target();
                                    let current = targets.keyboard_enter(surface.as_ref().id());
                                    (previous, current)
                                };
                                if previous_window != window_id {
                                    enqueue_for_window(
                                        &ev,
                                        previous_window,
                                        UiEvent::new(UiEventType::WindowBlur, UiEventPayload::None),
                                    );
                                    enqueue_for_window(
                                        &ev,
                                        window_id,
                                        UiEvent::new(
                                            UiEventType::WindowFocus,
                                            UiEventPayload::None,
                                        ),
                                    );
                                }
                                kd.lock().unwrap_or_else(|error| error.into_inner()).clear();
                                *held_key_info
                                    .lock()
                                    .unwrap_or_else(|error| error.into_inner()) = None;
                                *last_repeat_time
                                    .lock()
                                    .unwrap_or_else(|error| error.into_inner()) = None;
                            }
                            wl_keyboard::Event::Leave { surface, .. } => {
                                let blurred_window = {
                                    let mut targets =
                                        targets.lock().unwrap_or_else(|error| error.into_inner());
                                    let previous = targets.keyboard_target();
                                    targets
                                        .keyboard_leave(surface.as_ref().id())
                                        .then_some(previous)
                                        .flatten()
                                };
                                let left_focused_surface = blurred_window.is_some();
                                if left_focused_surface {
                                    enqueue_for_window(
                                        &ev,
                                        blurred_window,
                                        UiEvent::new(UiEventType::WindowBlur, UiEventPayload::None),
                                    );
                                    kd.lock().unwrap_or_else(|error| error.into_inner()).clear();
                                    *held_key_info
                                        .lock()
                                        .unwrap_or_else(|error| error.into_inner()) = None;
                                    *last_repeat_time
                                        .lock()
                                        .unwrap_or_else(|error| error.into_inner()) = None;
                                }
                            }
                            wl_keyboard::Event::Key {
                                serial, key, state, ..
                            } => {
                                let Some(window_id) = targets
                                    .lock()
                                    .unwrap_or_else(|error| error.into_inner())
                                    .keyboard_target()
                                else {
                                    return;
                                };
                                let code = linux_keycode_to_keycode(key);
                                let mut q = ev.lock().unwrap_or_else(|e| e.into_inner());
                                let current_mods = mods.lock().map(|m| *m).unwrap_or(KeyMod::NONE);
                                let shift_down = current_mods.intersects(KeyMod::SHIFT);
                                if state == wl_keyboard::KeyState::Pressed {
                                    // 去重：若已启用客户端侧重复且该键已被按下，
                                    // 跳过 compositor 发送的重复 Key 事件，避免双重重复
                                    let client_repeat_enabled =
                                        *repeat_rate.lock().unwrap_or_else(|e| e.into_inner()) > 0;
                                    let already_down =
                                        kd.lock().map(|ks| ks.contains(&code)).unwrap_or(false);
                                    if client_repeat_enabled && already_down {
                                        // compositor 侧重复，由客户端自行处理
                                        return;
                                    }
                                    keyboard_serial
                                        .lock()
                                        .unwrap_or_else(|error| error.into_inner())
                                        .record(serial);
                                    // 记录按住的键，用于客户端侧重复
                                    if let Ok(mut kd) = kd.lock() {
                                        kd.insert(code);
                                    }
                                    q.push_back(
                                        UiEvent::key_down(code, current_mods).for_window(window_id),
                                    );
                                    // 始终从物理键盘生成字符事件（即使 IME 激活）
                                    // IME 通过 CommitString 额外提交文本（如中文），两者互补
                                    if let Some(text) = keycode_to_char(code, shift_down) {
                                        q.push_back(
                                            UiEvent::text_input(text).for_window(window_id),
                                        );
                                    }
                                    // 记录按住的键，用于客户端侧重复
                                    if let Ok(mut hki) = held_key_info.lock() {
                                        *hki = Some(HeldKeyInfo {
                                            code,
                                            mods: current_mods,
                                            first_press: std::time::Instant::now(),
                                            window_id,
                                        });
                                    }
                                    if let Ok(mut lrt) = last_repeat_time.lock() {
                                        *lrt = None;
                                    }
                                } else {
                                    if let Ok(mut kd) = kd.lock() {
                                        kd.remove(&code);
                                    }
                                    q.push_back(
                                        UiEvent::key_up(code, current_mods).for_window(window_id),
                                    );
                                    // 释放键时清除重复跟踪
                                    if let Ok(mut hki) = held_key_info.lock() {
                                        *hki = None;
                                    }
                                    if let Ok(mut lrt) = last_repeat_time.lock() {
                                        *lrt = None;
                                    }
                                }
                            }
                            wl_keyboard::Event::Modifiers {
                                mods_depressed,
                                mods_latched,
                                mods_locked,
                                ..
                            } => {
                                if let Ok(mut m) = mods.lock() {
                                    let combined = mods_depressed | mods_latched | mods_locked;
                                    *m = KeyMod::NONE;
                                    if combined & 1 != 0 {
                                        *m |= KeyMod::SHIFT;
                                    }
                                    if combined & 4 != 0 {
                                        *m |= KeyMod::CTRL;
                                    }
                                    if combined & 8 != 0 {
                                        *m |= KeyMod::ALT;
                                    }
                                    if combined & 16 != 0 {
                                        *m |= KeyMod::SUPER;
                                    }
                                }
                                // 修饰键变化时重置重复状态
                                if let Ok(mut hki) = held_key_info.lock() {
                                    *hki = None;
                                }
                                if let Ok(mut lrt) = last_repeat_time.lock() {
                                    *lrt = None;
                                }
                            }
                            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                                if let Ok(mut rr) = repeat_rate.lock() {
                                    *rr = rate;
                                }
                                if let Ok(mut rd) = repeat_delay.lock() {
                                    *rd = delay;
                                }
                            }
                            _ => {}
                        }
                    });
                    if let Ok(mut k) = wl_keyboard_handle.lock() {
                        *k = Some(kbd);
                    }
                } else {
                    let blurred_window = {
                        let mut targets = surface_windows
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        let previous = targets.keyboard_target();
                        targets.clear_keyboard_focus();
                        previous
                    };
                    enqueue_for_window(
                        &ptr_events,
                        blurred_window,
                        UiEvent::new(UiEventType::WindowBlur, UiEventPayload::None),
                    );
                    keys_down
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .clear();
                    *held_key_info
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) = None;
                    *last_repeat_time
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) = None;
                    if let Ok(mut k) = wl_keyboard_handle.lock() {
                        *k = None;
                    }
                }
            }
        });

        self.seat = Some(seat);

        // ── 分发 Capabilities 事件 ─────────────────────────
        let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});
        for _ in 0..5 {
            let has_pointer = self.pointer.lock().map(|p| p.is_some()).unwrap_or(false);
            if has_pointer {
                break;
            }
            let _ = self.display.flush();
            let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});
        }
    }
}
