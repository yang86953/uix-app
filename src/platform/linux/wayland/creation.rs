// ============================================================================
// platform/linux/wayland/creation.rs — 窗口创建（Wayland 协议初始化与回调绑定）
// ============================================================================

use std::sync::{Arc, Mutex};

use wayland_client::protocol::{wl_data_device, wl_keyboard, wl_pointer, wl_seat, wl_surface};
use wayland_protocols::misc::server_decoration::client::{
    org_kde_kwin_server_decoration::Mode,
    org_kde_kwin_server_decoration_manager::OrgKdeKwinServerDecorationManager,
};
use wayland_protocols::unstable::xdg_decoration::v1::client::{
    zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
    zxdg_toplevel_decoration_v1::Mode as XdgDecoMode,
};
use wayland_protocols::xdg_shell::client::{xdg_surface, xdg_toplevel};

use crate::base::{KeyMod, MouseButton, Point};
use crate::diag::Error;
use crate::platform::event::*;
use crate::platform::linux::wayland::keycode::{keycode_to_char, linux_keycode_to_keycode};

use super::WaylandBackend;

impl WaylandBackend {
    /// 内部窗口创建方法（包含 Wayland 协议初始化与回调绑定）。
    pub(crate) fn create_window_inner(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<(), Error> {
        self.width = width;
        self.height = height;

        let events = self.events.clone();
        let last_pointer = self.last_pointer.clone();

        let surface = self._compositor.create_surface();
        let xdg_surface = self._wm_base.get_xdg_surface(&surface);
        let toplevel = xdg_surface.get_toplevel();
        toplevel.set_title(title.to_string());
        toplevel.set_app_id("uix-app".to_string());

        xdg_surface.quick_assign(move |xs, event, _| {
            if let xdg_surface::Event::Configure { serial } = event {
                xs.ack_configure(serial);
            }
        });

        // xdg_toplevel 事件 → UiEvent
        let toplevel_events = events.clone();
        let maximized_state = self.maximized.clone();
        let fullscreen_state = self.fullscreen.clone();
        toplevel.quick_assign(move |_, event, _| {
            match event {
                xdg_toplevel::Event::Close => {
                    log::debug!("xdg_toplevel::Close received");
                    let _ = toplevel_events
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push_back(UiEvent::close());
                }
                xdg_toplevel::Event::Configure {
                    width: w,
                    height: h,
                    states,
                } => {
                    let is_maximized = states
                        .chunks_exact(4)
                        .any(|c| c.len() == 4 && u32::from_ne_bytes([c[0], c[1], c[2], c[3]]) == 1);
                    let is_fullscreen = states
                        .chunks_exact(4)
                        .any(|c| c.len() == 4 && u32::from_ne_bytes([c[0], c[1], c[2], c[3]]) == 2);
                    let was_maximized = maximized_state.lock().map(|m| *m).unwrap_or(false);
                    if let Ok(mut m) = maximized_state.lock() {
                        *m = is_maximized;
                    }
                    if let Ok(mut f) = fullscreen_state.lock() {
                        *f = is_fullscreen;
                    }
                    log::debug!(
                        "xdg_toplevel::Configure w={} h={} max={} full={}",
                        w, h, is_maximized, is_fullscreen
                    );
                    if w > 0 && h > 0 {
                        let _ = toplevel_events
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push_back(UiEvent::resize(w, h));
                    }
                    if is_maximized && !was_maximized {
                        let _ = toplevel_events
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push_back(UiEvent {
                                type_: UiEventType::WindowMaximize,
                                payload: UiEventPayload::None,
                            });
                    } else if !is_maximized && was_maximized {
                        let _ = toplevel_events
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push_back(UiEvent {
                                type_: UiEventType::WindowRestore,
                                payload: UiEventPayload::None,
                            });
                    }
                }
                _ => {}
            }
        });

        // Seat: 指针 + 键盘
        if let Ok(seat) = self._globals.instantiate_exact::<wl_seat::WlSeat>(1) {
            // 剪贴板数据设备
            if let Some(ref sdm) = self.data_device_manager {
                let dev = sdm.get_data_device(&seat);
                let crfd = self.clipboard_read_fd.clone();
                dev.quick_assign(move |_, event, _| {
                    match event {
                        wl_data_device::Event::DataOffer { .. } => {}
                        wl_data_device::Event::Selection { id } => {
                            if let Some(offer) = id {
                                let mut fds = [0i32; 2];
                                let ret = unsafe { libc::pipe(fds.as_mut_ptr()) };
                                if ret == 0 {
                                    let read_fd = fds[0];
                                    offer.receive("text/plain;charset=utf-8".to_string(), fds[1]);
                                    if let Ok(mut rf) = crfd.lock() {
                                        *rf = Some(read_fd);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                });
                self.data_device = Some(dev);
            }

            let ptr_events = self.events.clone();
            let ptr_pos = last_pointer.clone();
            let keys_down = self.keys_down.clone();
            let wl_pointer_handle = self.pointer.clone();
            let wl_keyboard_handle = self.keyboard.clone();
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
                        let kbd = seat.get_keyboard();
                        kbd.quick_assign(move |_, event, _| {
                            match event {
                                wl_keyboard::Event::Key { key, state, .. } => {
                                    let code = linux_keycode_to_keycode(key);
                                    let mut q = ev.lock().unwrap_or_else(|e| e.into_inner());
                                    let current_mods = mods.lock().map(|m| *m).unwrap_or(KeyMod::NONE);
                                    let shift_down = current_mods.intersects(KeyMod::SHIFT);
                                    if let Ok(mut kd) = kd.lock() {
                                        if state == wl_keyboard::KeyState::Pressed { kd.insert(code); } else { kd.remove(&code); }
                                    }
                                    if state == wl_keyboard::KeyState::Pressed {
                                        q.push_back(UiEvent::key_down(code, current_mods));
                                        if let Some(text) = keycode_to_char(code, shift_down) {
                                            q.push_back(UiEvent::key_press(text));
                                        }
                                    } else {
                                        q.push_back(UiEvent::key_up(code, current_mods));
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
        }

        // 窗口装饰：KDE 协议优先，xdg-decoration 回退
        let got_decorations = self
            ._globals
            .instantiate_exact::<OrgKdeKwinServerDecorationManager>(1)
            .map(|dm| {
                let deco = dm.create(&surface);
                deco.request_mode(Mode::Server);
                self.decoration_manager = Some(dm);
                self.decoration = Some(deco);
                true
            })
            .unwrap_or(false);
        if !got_decorations {
            self._globals
                .instantiate_exact::<ZxdgDecorationManagerV1>(1)
                .map(|dm| {
                    let deco = dm.get_toplevel_decoration(&toplevel);
                    deco.set_mode(XdgDecoMode::ServerSide);
                    self.xdg_decoration_manager = Some(dm);
                    self.xdg_toplevel_decoration = Some(deco);
                    log::info!("Wayland: using xdg-decoration (server-side)");
                })
                .unwrap_or_else(|_| {
                    log::warn!("Wayland: no decoration protocol available");
                });
        }

        xdg_surface.set_window_geometry(0, 0, width, height);

        {
            let region = self._compositor.create_region();
            region.add(0, 0, width, height);
            surface.set_input_region(Some(&region));
            self.input_region = Some(region);
        }

        surface.commit();

        self.surface = Some(surface);
        self.xdg_surface = Some(xdg_surface);
        self.toplevel = Some(toplevel);

        // 首次 dispatch：flush 所有待发请求（包括 seat 绑定），接收 Capabilities 和 configure 事件
        let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});

        // 确保 pointer 被创建：如果 dispatch 未处理 Capabilities 事件（compositor 响应延迟），
        // 额外 flush + dispatch 几次，直到 pointer 可用或超时。
        // 若无 pointer，后续所有鼠标事件都会被静默丢弃。
        for _ in 0..5 {
            let has_pointer = self.pointer.lock()
                .map(|p| p.is_some())
                .unwrap_or(false);
            if has_pointer {
                break;
            }
            let _ = self.display.flush();
            let _ = self.event_queue.dispatch(&mut (), |_, _, _| {});
        }

        {
            let has_pointer = self.pointer.lock()
                .map(|p| p.is_some())
                .unwrap_or(false);
            if !has_pointer {
                log::warn!("Wayland: pointer 未创建（seat 无 Pointer capability），鼠标事件不可用");
            }
        }

        if let Some(ref s) = self.surface { s.commit(); }

        self.configured = true;
        Ok(())
    }
}
