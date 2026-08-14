// ============================================================================
// platform/linux/wayland/creation.rs — Seat 绑定与输入设备初始化
//
// 窗口创建（surface/toplevel/decoration 等）已迁移到 window_impl.rs。
// 此文件仅保留 WaylandBackend 的 seat 绑定（指针+键盘+剪贴板），
// 该绑定只需执行一次，多窗口共享。
// ============================================================================

use std::collections::VecDeque;
use std::os::fd::AsFd;
use std::sync::{Arc, Mutex};

use wayland_client::protocol::{wl_data_device, wl_keyboard, wl_pointer, wl_seat};
use wayland_client::{Proxy, WEnum};

use super::compat::Main;
// 引入完整 capability 快照到幂等代理边沿的纯决策。
use super::input_proxy_lifecycle::{input_proxy_transition, InputProxyTransition};
use super::{HeldKeyInfo, WaylandBackend};
use crate::core::{Errc, Error, Point, WindowId};
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

// 从 owner 槽取出 pointer 代理后，在锁外注销回调并按协议版本释放。
fn release_pointer_proxy(
    // 代理槽是 WaylandBackend 对 pointer 生命周期的唯一强 owner。
    pointer_slot: &Arc<Mutex<Option<Main<wl_pointer::WlPointer>>>>,
// 清理过程无返回值且重复调用保持幂等。
) {
    // 在短锁内只取走代理，禁止持锁执行协议请求或回调表操作。
    let pointer = pointer_slot
        // 获取唯一代理槽的可变访问。
        .lock()
        // 中毒时仍完成 owner-thread 确定性关闭。
        .unwrap_or_else(|error| error.into_inner())
        // 原子取走代理，后续重复调用得到 None。
        .take();
    // 只有实际存在的代理需要注销和释放。
    if let Some(pointer) = pointer {
        // 先删除兼容回调，迟到协议事件只能被安全忽略。
        pointer.clear_callback();
        // wl_pointer.release 从协议版本三开始可用。
        if pointer.version() >= 3 {
            // 在无代理槽锁的情况下提交释放请求。
            pointer.release();
        // 结束 pointer 协议版本保护。
        }
    // 结束 pointer 存在分支。
    }
// 结束 pointer 代理清理。
}

// 从 owner 槽取出 keyboard 代理后，在锁外注销回调并按协议版本释放。
fn release_keyboard_proxy(
    // 代理槽是 WaylandBackend 对 keyboard 生命周期的唯一强 owner。
    keyboard_slot: &Arc<Mutex<Option<Main<wl_keyboard::WlKeyboard>>>>,
// 清理过程无返回值且重复调用保持幂等。
) {
    // 在短锁内只取走代理，避免锁跨越协议或回调表边界。
    let keyboard = keyboard_slot
        // 获取唯一代理槽的可变访问。
        .lock()
        // 中毒时仍完成 owner-thread 确定性关闭。
        .unwrap_or_else(|error| error.into_inner())
        // 原子取走代理，后续重复调用得到 None。
        .take();
    // 只有实际存在的代理需要注销和释放。
    if let Some(keyboard) = keyboard {
        // 先删除兼容回调，迟到按键事件不能写入已关闭状态。
        keyboard.clear_callback();
        // wl_keyboard.release 从协议版本三开始可用。
        if keyboard.version() >= 3 {
            // 在无代理槽锁的情况下提交释放请求。
            keyboard.release();
        // 结束 keyboard 协议版本保护。
        }
    // 结束 keyboard 存在分支。
    }
// 结束 keyboard 代理清理。
}

impl WaylandBackend {
    // 由 backend owner 在 Drop 前统一关闭 seat 与输入代理生命周期。
    pub(crate) fn shutdown_seat_and_input(&mut self) {
        // 先撤销所有未消费授权并推进 pointer 代次，使迟到回调永久失效。
        self.pointer_activations
            // 获取授权注册表唯一可变访问。
            .lock()
            // 中毒时仍完成 owner-thread 关闭职责。
            .unwrap_or_else(|error| error.into_inner())
            // 清除授权并推进 pointer generation。
            .invalidate_pointer();
        // 原子清除共享输入焦点，防止关闭后保留旧窗口路由。
        {
            // 短时获取 surface 路由状态的唯一可变访问。
            let mut targets = self
                // 访问 backend 独占的共享路由表。
                .surface_windows
                // 获取清理锁。
                .lock()
                // 中毒时仍完成确定性焦点失效。
                .unwrap_or_else(|error| error.into_inner());
            // 清除 pointer focus。
            targets.clear_pointer_focus();
            // 清除 keyboard focus，但不伪造任何窗口模糊事件。
            targets.clear_keyboard_focus();
        // 结束 surface 路由锁作用域。
        }
        // 清除仍标记按下的键，避免 owner 关闭后保留输入状态。
        self.keys_down
            // 获取按键集合唯一可变访问。
            .lock()
            // 中毒时仍完成清理。
            .unwrap_or_else(|error| error.into_inner())
            // 丢弃全部按键状态。
            .clear();
        // 清除客户端按键重复持有信息。
        *self
            // 访问重复输入 owner 状态。
            .held_key_info
            // 获取唯一可变访问。
            .lock()
            // 中毒时仍完成关闭。
            .unwrap_or_else(|error| error.into_inner()) = None;
        // 清除上次重复时刻，使关闭状态不再产生定时事件。
        *self
            // 访问重复节拍状态。
            .last_repeat_time
            // 获取唯一可变访问。
            .lock()
            // 中毒时仍完成关闭。
            .unwrap_or_else(|error| error.into_inner()) = None;
        // 先从 owner 槽取走并释放 pointer 代理。
        release_pointer_proxy(&self.pointer);
        // 再对 keyboard 代理执行对称清理。
        release_keyboard_proxy(&self.keyboard);
        // 数据设备回调依赖 seat 生命周期，关闭时一并注销。
        if let Some(data_device) = self.data_device.take() {
            // 从兼容回调表删除数据设备闭包。
            data_device.clear_callback();
            // wl_data_device.release 从协议版本二开始可用。
            if data_device.version() >= 2 {
                // 在 owner 字段已取空后提交释放请求。
                data_device.release();
            // 结束 data device 协议版本保护。
            }
        // 结束 data device 存在分支。
        }
        // seat 最后注销，确保所有派生输入代理已先释放。
        if let Some(seat) = self.seat.take() {
            // 删除捕获输入槽的 seat callback，结构性打断兼容表强环。
            seat.clear_callback();
            // wl_seat.release 从协议版本五开始可用。
            if seat.version() >= 5 {
                // 在 owner 字段已取空后提交 seat 释放请求。
                seat.release();
            // 结束 seat 协议版本保护。
            }
        // 结束 seat 存在分支。
        }
    // 结束 backend 输入生命周期关闭。
    }

    /// 确保 seat 已绑定（指针 + 键盘 + 剪贴板数据设备）。
    /// 多窗口共享同一份输入设备绑定，仅首次调用时执行实际绑定。
    pub(crate) fn ensure_seat_and_input(&mut self) {
        if self.seat.is_some() {
            return; // 已绑定
        }

        let Some(global) = self
            ._globals
            .contents()
            .clone_list()
            .into_iter()
            .find(|global| global.interface == "wl_seat")
        else {
            tracing::warn!("Wayland: no wl_seat available, input unavailable");
            return;
        };
        let queue_handle = self._compositor.queue_handle();
        let seat = Main::new(
            self._globals.registry().bind::<wl_seat::WlSeat, _, _>(
                global.name,
                global.version.min(5),
                &queue_handle,
                (),
            ),
            self._compositor.context(),
        );

        // ── 剪贴板数据设备 ──────────────────────────────────
        if let Some(ref sdm) = self.data_device_manager {
            let dev = sdm.get_data_device(&seat);
            let clipboard_read = self.clipboard_read.clone();
            let clipboard_text = self.clipboard_text.clone();
            let owns_clipboard = self.owns_clipboard.clone();
            let pending_failures = self.pending_failures.clone();
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
                                    write_fd.as_fd(),
                                );
                                *clipboard_read
                                    .lock()
                                    .unwrap_or_else(|error| error.into_inner()) = Some(read);
                            }
                            Err(error) => {
                                let _ = pending_failures.enqueue(Error::new(
                                    Errc::IoError,
                                    format!("Wayland clipboard pipe creation failed: {error}"),
                                ));
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
        // seat callback 只弱引用 pointer owner 槽，避免 callback 表反向形成强环。
        let wl_pointer_handle = Arc::downgrade(&self.pointer);
        // keyboard owner 槽采用同一弱引用策略。
        let wl_keyboard_handle = Arc::downgrade(&self.keyboard);
        // 按键重复（客户端侧实现）
        let repeat_rate = self.repeat_rate.clone();
        let repeat_delay = self.repeat_delay.clone();
        let held_key_info = self.held_key_info.clone();
        let last_repeat_time = self.last_repeat_time.clone();
        let pointer_serial = self.last_input_serial.clone();
        let keyboard_serial = self.last_input_serial.clone();
        let surface_windows = self.surface_windows.clone();
        // 所有 pointer 回调共享 Wayland 后端唯一的激活授权注册表。
        let pointer_activations = self.pointer_activations.clone();
        seat.quick_assign(move |seat, event, _| {
            if let wl_seat::Event::Capabilities { capabilities } = event {
                use wayland_client::protocol::wl_seat::Capability;
                let WEnum::Value(capabilities) = capabilities else {
                    return;
                };
                // backend 已关闭时弱 pointer 槽无法升级，迟到 callback 安全终止。
                let Some(wl_pointer_handle) = wl_pointer_handle.upgrade() else {
                    // owner 消失后不得重新创建输入代理。
                    return;
                // 结束 pointer owner 缺失分支。
                };
                // backend 已关闭时弱 keyboard 槽同样无法升级。
                let Some(wl_keyboard_handle) = wl_keyboard_handle.upgrade() else {
                    // owner 消失后不得重新创建键盘代理。
                    return;
                // 结束 keyboard owner 缺失分支。
                };
                // Capabilities 是完整快照，先读取当前 pointer 代理是否已经绑定。
                let pointer_is_bound = wl_pointer_handle
                    // 短时锁定代理槽位读取边沿状态。
                    .lock()
                    // 中毒时仍以槽位现值完成 owner-thread 生命周期判断。
                    .unwrap_or_else(|error| error.into_inner())
                    // 只投影代理存在性，不延长额外锁作用域。
                    .is_some();
                // 将完整 pointer capability 快照映射为唯一幂等边沿。
                let pointer_transition = input_proxy_transition(
                    // 读取 compositor 本次是否声明 pointer 能力。
                    capabilities.contains(Capability::Pointer),
                    // 结合 owner 当前是否持有代理。
                    pointer_is_bound,
                // 结束 pointer 边沿输入。
                );
                // 读取当前 keyboard 代理槽以计算同一快照的键盘边沿。
                let keyboard_is_bound = wl_keyboard_handle
                    // 短时锁定 keyboard owner 槽。
                    .lock()
                    // 中毒时仍以槽位现值完成生命周期判断。
                    .unwrap_or_else(|error| error.into_inner())
                    // 只投影代理存在性。
                    .is_some();
                // 将完整 keyboard capability 快照映射为唯一幂等边沿。
                let keyboard_transition = input_proxy_transition(
                    // 读取 compositor 本次是否声明 keyboard 能力。
                    capabilities.contains(Capability::Keyboard),
                    // 结合 owner 当前是否持有代理。
                    keyboard_is_bound,
                // 结束 keyboard 边沿输入。
                );
                // 仅在 Pointer 能力从无到有时创建一个代理与回调。
                if pointer_transition == InputProxyTransition::Bind {
                    let ev = ptr_events.clone();
                    let pos = ptr_pos.clone();
                    let pointer_serial = pointer_serial.clone();
                    let targets = surface_windows.clone();
                    // 新回调持有共享激活注册表句柄。
                    let pointer_activations = pointer_activations.clone();
                    // 捕获创建该 pointer 代理时的稳定代次。
                    let pointer_generation = pointer_activations
                        // 短时锁定注册表读取当前代次。
                        .lock()
                        // 中毒时仍恢复 owner-thread 可用状态。
                        .unwrap_or_else(|error| error.into_inner())
                        // 复制代次供所有迟到回调校验。
                        .pointer_generation();
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
                                .pointer_enter(surface.id().protocol_id());
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
                            // 保存离开事件携带的精确 surface 协议身份。
                            let surface_id = surface.id().protocol_id();
                            // 在同一 target 锁内读取旧焦点并完成清理。
                            let window_id = {
                                // 短时锁定共享 surface 路由状态。
                                let mut targets = targets
                                    // 获取焦点与注册映射的唯一可变访问。
                                    .lock()
                                    // 中毒时继续执行确定性 owner-thread 清理。
                                    .unwrap_or_else(|error| error.into_inner());
                                // 仅接受确实指向本次 leave surface 的旧焦点。
                                let window_id = targets
                                    // 原子投影 surface 与窗口身份。
                                    .pointer_target_identity()
                                    // 排除其他 surface 的迟到 leave。
                                    .filter(|(focused_surface, _)| *focused_surface == surface_id)
                                    // 只保留撤销授权所需的窗口身份。
                                    .map(|(_, window_id)| window_id);
                                // 清除共享 pointer focus，重复 leave 保持幂等。
                                targets.pointer_leave(surface_id);
                                // 返回 leave 前的精确窗口身份。
                                window_id
                            // 结束 surface 路由锁作用域。
                            };
                            // 只有已知焦点 surface 才能撤销对应待消费授权。
                            if let Some(window_id) = window_id {
                                // 在独立短锁中按 pointer 代次与窗口 surface 撤销。
                                pointer_activations
                                    // 获取激活注册表唯一可变访问。
                                    .lock()
                                    // 中毒时仍完成授权失效。
                                    .unwrap_or_else(|error| error.into_inner())
                                    // 防止旧代理 leave 撤销新代次授权。
                                    .revoke_pointer_focus(
                                        // 使用创建回调时捕获的 pointer 代次。
                                        pointer_generation,
                                        // 使用离开事件的 surface 身份。
                                        surface_id,
                                        // 使用离开前解析出的稳定窗口身份。
                                        window_id,
                                    // 结束精确撤销参数。
                                    );
                            // 结束已知焦点撤销分支。
                            }
                        }
                        wl_pointer::Event::Button {
                            serial,
                            button,
                            state,
                            ..
                        } => {
                            // 原子读取 press/release 当时的 surface 与窗口焦点身份。
                            let Some((surface_id, window_id)) = targets
                                // 短时锁定共享 surface 路由状态。
                                .lock()
                                // 中毒时仍恢复 owner-thread 输入路由。
                                .unwrap_or_else(|error| error.into_inner())
                                // 同时投影 surface 和稳定窗口身份。
                                .pointer_target_identity()
                            // 未知焦点事件不得签发或撤销任何窗口授权。
                            else {
                                // 丢弃无法定向的按键事件。
                                return;
                            // 结束未知焦点分支。
                            };
                            // 将 Linux 输入按钮编号映射为平台中立按钮。
                            let btn = match button {
                                // BTN_LEFT 是唯一允许签发窗口拖动授权的主键。
                                0x110 => MouseButton::Left,
                                // 右键继续服务系统菜单等普通 UI 语义。
                                0x111 => MouseButton::Right,
                                // 中键继续作为普通指针输入。
                                0x112 => MouseButton::Middle,
                                // 未知按钮不具备拖动授权语义。
                                _ => MouseButton::None,
                            // 结束原生按钮映射。
                            };
                            // 捕获按键事件发生时的最近 surface 坐标。
                            let click_pos = pos
                                // 短时读取共享指针位置。
                                .lock()
                                // 锁失败时使用默认坐标保持既有降级语义。
                                .map(|last_pointer| last_pointer.position)
                                // 位置读取失败不影响授权身份校验。
                                .unwrap_or_default();
                            // 明确区分协议已知的 press、release 与未知状态。
                            match state {
                                // press serial 仍可供剪贴板等现有输入授权使用。
                                WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                                    // 更新通用输入 serial，但窗口拖动不读取该全局槽位。
                                    pointer_serial
                                        // 短时锁定既有输入 serial 状态。
                                        .lock()
                                        // 中毒时仍保留最新合法协议 serial。
                                        .unwrap_or_else(|error| error.into_inner())
                                        // 记录 compositor 实际签发值，绝不伪造零。
                                        .record(serial);
                                    // 先构造不携带平台细节的普通 PointerDown。
                                    let pointer_event = UiEvent::pointer_down(click_pos, btn);
                                    // 只有 BTN_LEFT press 才尝试签发一次性拖动激活身份。
                                    let pointer_event = if btn == MouseButton::Left {
                                        // 在短锁内把 raw serial 绑定到窗口与 surface 代次。
                                        let activation = pointer_activations
                                            // 获取激活注册表唯一可变访问。
                                            .lock()
                                            // 中毒时仍由 owner thread 保持授权边界。
                                            .unwrap_or_else(|error| error.into_inner())
                                            // 使用回调代次阻止旧 pointer 代理签发新授权。
                                            .issue_primary_press(
                                                // 传入创建回调时捕获的 pointer 代次。
                                                pointer_generation,
                                                // 绑定当前焦点 surface 身份。
                                                surface_id,
                                                // 绑定当前焦点窗口身份。
                                                window_id,
                                                // raw serial 只进入 Wayland 私有注册表。
                                                serial,
                                            // 结束授权签发参数。
                                            );
                                        // 将不可解释身份附着到同一个 native 事件。
                                        match activation {
                                            // 完整匹配当前注册时附着身份。
                                            Some(activation) => pointer_event
                                                // UI 映射层不会解释该身份。
                                                .with_pointer_activation(activation),
                                            // 路由或代次不匹配时仍交付普通按下事件。
                                            None => pointer_event,
                                        // 结束激活身份附着分支。
                                        }
                                    // 非主键永远没有窗口拖动授权。
                                    } else {
                                        // 原样保留普通指针事件。
                                        pointer_event
                                    // 结束非主键分支。
                                    };
                                    // 把事件定向到授权绑定的同一窗口。
                                    enqueue_for_window(
                                        // 使用共享 UI 事件队列。
                                        &ev,
                                        // 使用焦点快照的稳定窗口身份。
                                        Some(window_id),
                                        // 交付普通事件及可选不透明身份。
                                        pointer_event,
                                    // 结束定向入队参数。
                                    );
                                // 结束 press 分支。
                                }
                                // release 必须撤销尚未消费的主键协议授权。
                                WEnum::Value(wl_pointer::ButtonState::Released) => {
                                    // 仅 BTN_LEFT release 影响拖动授权生命周期。
                                    if btn == MouseButton::Left {
                                        // 在独立短锁内撤销同一 pointer 代次授权。
                                        pointer_activations
                                            // 获取注册表唯一可变访问。
                                            .lock()
                                            // 中毒时仍完成授权撤销。
                                            .unwrap_or_else(|error| error.into_inner())
                                            // 旧代理 release 不得影响新代次授权。
                                            .revoke_primary_press(pointer_generation);
                                    // 结束主键 release 撤销。
                                    }
                                    // release 仍按既有 UI 输入语义定向交付。
                                    enqueue_for_window(
                                        // 使用共享 UI 事件队列。
                                        &ev,
                                        // 使用 release 当时的焦点窗口身份。
                                        Some(window_id),
                                        // 抬起事件不携带可复用激活身份。
                                        UiEvent::pointer_up(click_pos, btn),
                                    // 结束 release 入队参数。
                                    );
                                // 结束 release 分支。
                                }
                                // 未知协议状态不能被当作 release 或 press。
                                _ => {
                                    // 丢弃无法安全解释的按钮状态（该分支位于状态分派尾部，直接结束即可）。
                                // 结束未知状态分支。
                                }
                            // 结束按钮状态分派。
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
                                WEnum::Value(wl_pointer::Axis::VerticalScroll) => (0.0, value),
                                WEnum::Value(wl_pointer::Axis::HorizontalScroll) => (value, 0.0),
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
                    // owner 槽在回调执行期间已由升级后的强引用保证存活。
                    *wl_pointer_handle
                        // 获取唯一代理槽的可变访问。
                        .lock()
                        // 中毒时仍恢复 owner-thread 绑定职责。
                        .unwrap_or_else(|error| error.into_inner()) = Some(ptr);
                // 仅在 Pointer 能力从有到无时释放代理并推进授权代次。
                } else if pointer_transition == InputProxyTransition::Release {
                    // capability loss 先撤销全部未消费授权并使旧回调代次失效。
                    pointer_activations
                        // 获取注册表唯一可变访问。
                        .lock()
                        // 中毒时仍完成 owner-thread 生命周期失效。
                        .unwrap_or_else(|error| error.into_inner())
                        // 清空授权并推进 pointer generation。
                        .invalidate_pointer();
                    // pointer 能力消失时同步清除共享 surface 焦点。
                    surface_windows
                        // 短时锁定路由状态。
                        .lock()
                        // 中毒时继续完成确定性清理。
                        .unwrap_or_else(|error| error.into_inner())
                        // 防止后续无 surface 事件沿用旧窗口焦点。
                        .clear_pointer_focus();
                    // 统一 helper 在锁外注销回调并释放唯一 pointer 代理。
                    release_pointer_proxy(&wl_pointer_handle);
                // 结束 pointer capability 边沿处理。
                }
                // 仅在 Keyboard 能力从无到有时创建一个代理与回调。
                if keyboard_transition == InputProxyTransition::Bind {
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
                                    let current =
                                        targets.keyboard_enter(surface.id().protocol_id());
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
                                        .keyboard_leave(surface.id().protocol_id())
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
                                if state == WEnum::Value(wl_keyboard::KeyState::Pressed) {
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
                    // owner 槽在回调执行期间已由升级后的强引用保证存活。
                    *wl_keyboard_handle
                        // 获取唯一 keyboard 代理槽的可变访问。
                        .lock()
                        // 中毒时仍恢复 owner-thread 绑定职责。
                        .unwrap_or_else(|error| error.into_inner()) = Some(kbd);
                // 仅在 Keyboard 能力从有到无时清理状态并释放代理。
                } else if keyboard_transition == InputProxyTransition::Release {
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
                    // 统一 helper 在锁外注销回调并释放唯一 keyboard 代理。
                    release_keyboard_proxy(&wl_keyboard_handle);
                }
            }
        });

        self.seat = Some(seat);

        // ── 分发 Capabilities 事件 ─────────────────────────
        if !self.dispatch_pending_checked("seat capabilities") {
            return;
        }
        for _ in 0..5 {
            let has_pointer = self.pointer.lock().map(|p| p.is_some()).unwrap_or(false);
            if has_pointer {
                break;
            }
            if !self.flush_checked("seat capabilities")
                || !self.dispatch_pending_checked("seat capabilities")
            {
                break;
            }
        }
    }
}
