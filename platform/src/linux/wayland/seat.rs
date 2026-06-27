// ============================================================================
// platform/linux/wayland/creation.rs — Seat 绑定与输入设备初始化
//
// 窗口创建（surface/toplevel/decoration 等）已迁移到 window_impl.rs。
// 此文件仅保留 WaylandBackend 的 seat 绑定（指针+键盘+剪贴板），
// 该绑定只需执行一次，多窗口共享。
// ============================================================================

use std::sync::{Arc, Mutex};

use wayland_client::protocol::{wl_data_device, wl_keyboard, wl_pointer, wl_seat};

use crate::{KeyMod, MouseButton, Point};
use crate::event::*;
use crate::linux::wayland::keycode::{keycode_to_char, linux_keycode_to_keycode};

use super::WaylandBackend;

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
                log::warn!("Wayland: no wl_seat available, input unavailable");
                return;
            }
        };

        // ── 剪贴板数据设备 ──────────────────────────────────
        if let Some(ref sdm) = self.data_device_manager {
            let dev = sdm.get_data_device(&seat);
            let crfd = self.clipboard_read_fd.clone();
            dev.quick_assign(move |_, event, _| {
                if let wl_data_device::Event::Selection { id } = event {
                    if let Some(offer) = id {
                        let mut fds = [0i32; 2];
                        let ret = unsafe { libc::pipe(fds.as_mut_ptr()) };
                        if ret == 0 {
                            offer.receive("text/plain;charset=utf-8".to_string(), fds[1]);
                            if let Ok(mut rf) = crfd.lock() {
                                *rf = Some(fds[0]);
                            }
                        }
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
                    let _kbd = seat.get_keyboard();

        seat.quick_assign(move |seat, event, _| {
            if let wl_seat::Event::Capabilities { capabilities } = event {
                use wayland_client::protocol::wl_seat::Capability;
                if capabilities.contains(Capability::Pointer) {
                    let ev = ptr_events.clone();
                    let pos = ptr_pos.clone();
                    let ptr = seat.get_pointer();
                    ptr.quick_assign(move |_, event, _| {
                        let mut q = ev.lock().unwrap_or_else(|e| e.into_inner());
                        match event {
                            wl_pointer::Event::Enter { surface_x, surface_y, .. } => {
                                let p = Point::new(surface_x as f32, surface_y as f32);
                                if let Ok(mut lp) = pos.lock() { lp.position = p; }
                                q.push_back(UiEvent::mouse_move(p));
                            }
                            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                                let p = Point::new(surface_x as f32, surface_y as f32);
                                if let Ok(mut lp) = pos.lock() { lp.position = p; }
                                q.push_back(UiEvent::mouse_move(p));
                            }
                            wl_pointer::Event::Leave { .. } => {}
                            wl_pointer::Event::Button { button, state, .. } => {
                                let btn = match button {
                                    0x110 => MouseButton::Left,
                                    0x111 => MouseButton::Right,
                                    0x112 => MouseButton::Middle,
                                    _ => MouseButton::None,
                                };
                                let click_pos = pos.lock().map(|lp| lp.position).unwrap_or_default();
                                if state == wl_pointer::ButtonState::Pressed {
                                    q.push_back(UiEvent::mouse_down(click_pos, btn));
                                } else {
                                    q.push_back(UiEvent::mouse_up(click_pos, btn));
                                }
                            }
                            wl_pointer::Event::Axis { axis, value, .. } => {
                                let (dx, dy) = match axis {
                                    wl_pointer::Axis::VerticalScroll => (0.0, value),
                                    wl_pointer::Axis::HorizontalScroll => (value, 0.0),
                                    _ => (0.0, 0.0),
                                };
                                if dx != 0.0 || dy != 0.0 {
                                    let scroll_pos = pos.lock().map(|lp| lp.position).unwrap_or_default();
                                    q.push_back(UiEvent::mouse_wheel(scroll_pos, dx as f32, dy as f32, KeyMod::NONE));
                                }
                            }
                            _ => {}
                        }
                    });
                    if let Ok(mut p) = wl_pointer_handle.lock() { *p = Some(ptr); }
                } else {
                    if let Ok(mut p) = wl_pointer_handle.lock() { *p = None; }
                }
                if capabilities.contains(Capability::Keyboard) {
                    let ev = ptr_events.clone();
                    let kd = keys_down.clone();
                    let mods = Arc::new(Mutex::new(KeyMod::NONE));
                    let repeat_rate = repeat_rate.clone();
                    let repeat_delay = repeat_delay.clone();
                    let held_key_info = held_key_info.clone();
                    let last_repeat_time = last_repeat_time.clone();
                    let kbd = seat.get_keyboard();
                    kbd.quick_assign(move |_, event, _| {
                        match event {
                            wl_keyboard::Event::Key { key, state, .. } => {
                                let code = linux_keycode_to_keycode(key);
                                let mut q = ev.lock().unwrap_or_else(|e| e.into_inner());
                                let current_mods = mods.lock().map(|m| *m).unwrap_or(KeyMod::NONE);
                                let shift_down = current_mods.intersects(KeyMod::SHIFT);
                                if state == wl_keyboard::KeyState::Pressed {
                                    // 去重：若已启用客户端侧重复且该键已被按下，
                                    // 跳过 compositor 发送的重复 Key 事件，避免双重重复
                                    let client_repeat_enabled = *repeat_rate.lock().unwrap_or_else(|e| e.into_inner()) > 0;
                                    let already_down = kd.lock().map(|ks| ks.contains(&code)).unwrap_or(false);
                                    if client_repeat_enabled && already_down {
                                        // compositor 侧重复，由客户端自行处理
                                        return;
                                    }
                                    // 记录按住的键，用于客户端侧重复
                                    if let Ok(mut kd) = kd.lock() {
                                        kd.insert(code);
                                    }
                                    q.push_back(UiEvent::key_down(code, current_mods));
                                    // 始终从物理键盘生成字符事件（即使 IME 激活）
                                    // IME 通过 CommitString 额外提交文本（如中文），两者互补
                                    if let Some(text) = keycode_to_char(code, shift_down) {
                                        q.push_back(UiEvent::key_press(text));
                                    }
                                    // 记录按住的键，用于客户端侧重复
                                    if let Ok(mut hki) = held_key_info.lock() {
                                        *hki = Some((key, code, current_mods, std::time::Instant::now()));
                                    }
                                    if let Ok(mut lrt) = last_repeat_time.lock() {
                                        *lrt = None;
                                    }
                                } else {
                                    if let Ok(mut kd) = kd.lock() {
                                        kd.remove(&code);
                                    }
                                    q.push_back(UiEvent::key_up(code, current_mods));
                                    // 释放键时清除重复跟踪
                                    if let Ok(mut hki) = held_key_info.lock() {
                                        *hki = None;
                                    }
                                    if let Ok(mut lrt) = last_repeat_time.lock() {
                                        *lrt = None;
                                    }
                                }
                            }
                            wl_keyboard::Event::Modifiers { mods_depressed, mods_latched, mods_locked, .. } => {
                                if let Ok(mut m) = mods.lock() {
                                    let combined = mods_depressed | mods_latched | mods_locked;
                                    *m = KeyMod::NONE;
                                    if combined & 1 != 0 { *m |= KeyMod::SHIFT; }
                                    if combined & 4 != 0 { *m |= KeyMod::CTRL; }
                                    if combined & 8 != 0 { *m |= KeyMod::ALT; }
                                    if combined & 16 != 0 { *m |= KeyMod::SUPER; }
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
                    if let Ok(mut k) = wl_keyboard_handle.lock() { *k = Some(kbd); }
                } else {
                    if let Ok(mut k) = wl_keyboard_handle.lock() { *k = None; }
                }
            }
        });

        self.seat = Some(seat);

        // ── 分发 Capabilities 事件 ─────────────────────────
        let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});
        for _ in 0..5 {
            let has_pointer = self.pointer.lock().map(|p| p.is_some()).unwrap_or(false);
            if has_pointer { break; }
            let _ = self.display.flush();
            let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});
        }
    }
}