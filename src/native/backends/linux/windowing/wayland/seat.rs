// ============================================================================
// platform/linux/wayland/creation.rs — Seat 绑定与输入设备初始化
//
// 窗口创建（surface/toplevel/decoration 等）已迁移到 window_impl.rs。
// 此文件仅保留 WaylandBackend 的 seat 绑定（指针+键盘+剪贴板），
// 该绑定只需执行一次，多窗口共享。
// ============================================================================

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// data-device 事件在 seat Adapter 中只做 DnD/Selection owner 路由。
use wayland_client::protocol::{wl_data_device, wl_keyboard, wl_pointer, wl_seat};
use wayland_client::{Proxy, WEnum};

use super::compat::Main;
// 引入完整 capability 快照到幂等代理边沿的纯决策。
use super::input_proxy_lifecycle::{InputProxyTransition, input_proxy_transition};
// capability callback 通过检查式 Component 读取和提交代理 owners。
use super::input_proxy_owner::{
    // keyboard callback 注册完成后检查式安装。
    install_keyboard_proxy,
    // pointer callback 注册完成后检查式安装。
    install_pointer_proxy,
    // pointer callback 创建前检查 activation generation。
    pointer_generation_checked,
    // keyboard capability loss 事务化清理焦点、输入状态与代理。
    release_keyboard_proxy_checked,
    // pointer capability loss 事务化清理授权、焦点与代理。
    release_pointer_proxy_checked,
    // transition 决策前同时读取 pointer/keyboard 槽快照。
    snapshot_input_proxy_slots,
};
// keyboard Enter/Leave 委托焦点与重复状态事务 Component。
use super::keyboard_focus_owner::{
    // Enter 原子切换焦点并清理输入状态。
    handle_keyboard_enter,
    // Leave 原子清理精确焦点与输入状态。
    handle_keyboard_leave,
};
// keyboard Key 委托按键、serial 与重复状态事务 Component。
use super::keyboard_key_owner::{
    // Press 原子记录 serial、事件与客户端重复候选。
    handle_keyboard_key_pressed,
    // Release 原子提交 KeyUp 并清理重复状态。
    handle_keyboard_key_released,
};
// keyboard Modifiers 委托修饰快照与重复状态事务 Component。
use super::keyboard_modifier_owner::handle_keyboard_modifiers;
// keyboard RepeatInfo 委托 rate/delay 双 owner 事务 Component。
use super::keyboard_repeat_config_owner::handle_keyboard_repeat_info;
// pointer Button callback 委托多 owner 事务 Component。
use super::pointer_button_owner::{
    // Press 原子签发授权、记录 serial 并投递 PointerDown。
    handle_pointer_button_pressed,
    // Release 原子撤销授权并投递 PointerUp。
    handle_pointer_button_released,
};
// 指针轴帧 Component 统一离散滚轮与连续触控板距离。
use super::pointer_axis::PointerAxisFrame;
// pointer callback 把焦点与移动事件委托给多 owner 事务 Component。
use super::WaylandBackend;
use super::pointer_focus_owner::{
    // Axis 原子读取路由与位置并投递 Wheel。
    handle_pointer_axis,
    // Enter 原子提交 focus、position 与 PointerMove。
    handle_pointer_enter,
    // Leave 原子撤销授权并清除精确 surface focus。
    handle_pointer_leave,
    // Motion 原子提交 position 与 PointerMove。
    handle_pointer_motion,
};
// seat owner 保留几何与 capability 收敛的 typed failure。
use crate::core::{Errc, Error, Point};
// seat adapter 只把 Linux 键码转换为平台中立键码。
use crate::native::backends::linux::wayland::keycode::linux_keycode_to_keycode;
use crate::platform::windowing::{KeyMod, MouseButton};

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
        // backend shutdown 同时撤销不能跨代理生命周期复用的 cursor serial。
        self.cursor_state.clear_enter();
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
            // data-device callback 与窗口能力入口共享唯一文件拖放 Component。
            let file_drop_state = self.file_drop_state.clone();
            // Enter 使用既有 surface 路由表解析稳定 WindowId。
            let surface_windows = self.surface_windows.clone();
            let pending_failures = self.pending_failures.clone();
            dev.quick_assign(move |data_device, event, _| {
                // data-device Module 按协议事件种类路由到 DnD 或 Selection owner。
                match event {
                    // 新 offer 先注册 MIME/Action callback，等待后续 Enter 或 Selection 分类。
                    wl_data_device::Event::DataOffer { id } => {
                        // child Main 复用当前 data-device 的兼容上下文。
                        super::file_drop::register_data_offer(
                            // 传入 callback 当前协议 owner。
                            data_device,
                            // 转交 event-created child。
                            id,
                            // 转交文件拖放 Component。
                            &file_drop_state,
                            // 共享同一 failure source。
                            &pending_failures,
                        );
                    }
                    // Selection 从未分类 offer 中脱离后保持既有 clipboard 行为。
                    wl_data_device::Event::Selection { id } => {
                        // 清理 DnD offer callback，但不销毁 clipboard 即将读取的 proxy。
                        super::file_drop::detach_selection_offer(
                            // 只借用可选 offer 做 identity 匹配。
                            id.as_ref(),
                            // 访问唯一拖放 Component。
                            &file_drop_state,
                            // 状态失败进入既有 source。
                            &pending_failures,
                        );
                        // Selection 继续由 clipboard Module 独占业务状态。
                        super::clipboard::handle_selection_event(
                            // 重建被匹配消费的 Selection 事件。
                            wl_data_device::Event::Selection { id },
                            // 转交本地 ownership owner。
                            &owns_clipboard,
                            // 转交 read FD owner。
                            &clipboard_read,
                            // 转交文本缓存 owner。
                            &clipboard_text,
                            // 转交同一 backend pending source。
                            &pending_failures,
                        );
                    }
                    // Enter/Motion/Leave/Drop 只进入文件拖放 Module。
                    event => super::file_drop::handle_data_device_event(
                        // 转交完整 DnD 事件。
                        event,
                        // 解析 surface 到窗口身份。
                        &surface_windows,
                        // 更新会话/transfer Component。
                        &file_drop_state,
                        // 共享 callback failure source。
                        &pending_failures,
                    ),
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
        // pointer callback 共享唯一 cursor intent 与 Enter serial Component。
        let cursor_state = Arc::clone(&self.cursor_state);
        // 可选 cursor-shape global 在每次 Enter 重放当前 intent。
        let cursor_shape_manager = self.cursor_shape_manager.clone();
        // capability owner failure 复用 backend 已有 pending source。
        let capability_failures = self.pending_failures.clone();
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
                // Capabilities 是完整快照，先检查式读取两个代理 owner 槽。
                let Some((pointer_is_bound, keyboard_is_bound)) = snapshot_input_proxy_slots(
                    // pointer 槽必须先验证健康。
                    &wl_pointer_handle,
                    // keyboard 槽随后验证健康。
                    &wl_keyboard_handle,
                    // 任一 failure 进入同一 backend source。
                    &capability_failures,
                ) else {
                    // 槽损坏时不计算 transition、不创建协议代理。
                    return;
                };
                // 将完整 pointer capability 快照映射为唯一幂等边沿。
                let pointer_transition = input_proxy_transition(
                    // 读取 compositor 本次是否声明 pointer 能力。
                    capabilities.contains(Capability::Pointer),
                    // 结合 owner 当前是否持有代理。
                    pointer_is_bound,
                    // 结束 pointer 边沿输入。
                );
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
                    // pointer callback owner failure 复用 backend pending source。
                    let pointer_failures = capability_failures.clone();
                    // 新回调持有共享激活注册表句柄。
                    let pointer_activations = pointer_activations.clone();
                    // 新 pointer 代理捕获同一 cursor intent owner。
                    let pointer_cursor_state = Arc::clone(&cursor_state);
                    // cursor-shape global 与 backend 生命周期一致，可安全克隆代理 handle。
                    let pointer_cursor_shape_manager = cursor_shape_manager.clone();
                    // 捕获创建该 pointer 代理时的稳定代次。
                    let Some(pointer_generation) = pointer_generation_checked(
                        // generation 仍由 activation registry 唯一拥有。
                        &pointer_activations,
                        // registry failure 进入同一 backend source。
                        &capability_failures,
                    ) else {
                        // 状态失败时不调用 get_pointer 或注册 callback。
                        return;
                    };
                    let ptr = seat.get_pointer();
                    // 每个 pointer 代理独占一个协议帧累加器，释放代理时随回调一并销毁。
                    let pointer_axis_frame = Rc::new(RefCell::new(PointerAxisFrame::default()));
                    ptr.quick_assign(move |pointer, event, _| match event {
                        wl_pointer::Event::Enter {
                            serial,
                            surface,
                            surface_x,
                            surface_y,
                        } => {
                            // cursor Component 先记录本次 serial 并重放形状或隐藏 intent。
                            super::cursor::apply_cursor_on_pointer_enter(
                                // 使用事件所属的精确 pointer 代理。
                                pointer,
                                // 只使用本次 Enter 授权。
                                serial,
                                // 缺少 optional global 时保持 compositor 默认行为。
                                pointer_cursor_shape_manager.as_ref(),
                                // 传入 backend 唯一 cursor intent owner。
                                &pointer_cursor_state,
                                // 不变量失败进入同一 backend pending source。
                                &pointer_failures,
                            );
                            // adapter 只把 surface 身份与平台中立坐标转交 Component。
                            let p = Point::new(surface_x as f32, surface_y as f32);
                            // 三 owner Component 检查式提交焦点、位置与事件。
                            handle_pointer_enter(
                                // 协议 surface 身份只用于原生路由。
                                surface.id().protocol_id(),
                                // 平台中立坐标进入共享输入状态。
                                p,
                                // surface focus owner。
                                &targets,
                                // 最近位置 owner。
                                &pos,
                                // UI 事件队列 owner。
                                &ev,
                                // failure 进入 backend pending source。
                                &pointer_failures,
                            );
                        }
                        wl_pointer::Event::Motion {
                            surface_x,
                            surface_y,
                            ..
                        } => {
                            // adapter 只把平台中立坐标转交 Component。
                            let p = Point::new(surface_x as f32, surface_y as f32);
                            // 三 owner Component 检查式提交位置与事件。
                            handle_pointer_motion(
                                // 平台中立坐标进入共享输入状态。
                                p,
                                // surface focus owner。
                                &targets,
                                // 最近位置 owner。
                                &pos,
                                // UI 事件队列 owner。
                                &ev,
                                // failure 进入 backend pending source。
                                &pointer_failures,
                            );
                        }
                        wl_pointer::Event::Leave { surface, .. } => {
                            // 保存离开事件携带的精确 surface 协议身份。
                            let surface_id = surface.id().protocol_id();
                            // 双 owner Component 沿全局顺序撤销授权并清除焦点。
                            handle_pointer_leave(
                                // 使用事件携带的精确 surface 身份。
                                surface_id,
                                // 使用 callback 创建时捕获的 pointer 代次。
                                pointer_generation,
                                // activation registry 是全局锁序的第一 owner。
                                &pointer_activations,
                                // surface focus 是全局锁序的第二 owner。
                                &targets,
                                // failure 进入 backend pending source。
                                &pointer_failures,
                            );
                            // 协议 Leave 已发生，无论 UI focus owner 是否健康都撤销 cursor serial。
                            super::cursor::clear_cursor_pointer_focus(&pointer_cursor_state);
                        }
                        wl_pointer::Event::Button {
                            serial,
                            button,
                            state,
                            ..
                        } => {
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
                            // 明确区分协议已知的 press、release 与未知状态。
                            match state {
                                // press serial 仍可供剪贴板等现有输入授权使用。
                                WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                                    // 多 owner Component 在全部 guards 健康后提交 Press。
                                    handle_pointer_button_pressed(
                                        // raw serial 只传入 Wayland 私有 Component。
                                        serial,
                                        // 传入平台中立按钮语义。
                                        btn,
                                        // 使用 callback 创建时捕获的 pointer 代次。
                                        pointer_generation,
                                        // activation registry 是主键全局第一 owner。
                                        &pointer_activations,
                                        // surface focus owner。
                                        &targets,
                                        // 最近位置 owner。
                                        &pos,
                                        // UI 事件队列 owner。
                                        &ev,
                                        // 共享输入 serial owner。
                                        &pointer_serial,
                                        // failure 进入 backend pending source。
                                        &pointer_failures,
                                    );
                                }
                                // release 必须撤销尚未消费的主键协议授权。
                                WEnum::Value(wl_pointer::ButtonState::Released) => {
                                    // 多 owner Component 在全部 guards 健康后提交 Release。
                                    handle_pointer_button_released(
                                        // 传入平台中立按钮语义。
                                        btn,
                                        // 使用 callback 创建时捕获的 pointer 代次。
                                        pointer_generation,
                                        // activation registry 是主键全局第一 owner。
                                        &pointer_activations,
                                        // surface focus owner。
                                        &targets,
                                        // 最近位置 owner。
                                        &pos,
                                        // UI 事件队列 owner。
                                        &ev,
                                        // failure 进入 backend pending source。
                                        &pointer_failures,
                                    );
                                }
                                // 未知协议状态不能被当作 release 或 press。
                                _ => {
                                    // 丢弃无法安全解释的按钮状态（该分支位于状态分派尾部，直接结束即可）。
                                    // 结束未知状态分支。
                                } // 结束按钮状态分派。
                            }
                        }
                        wl_pointer::Event::Axis { axis, value, .. } => {
                            // 先按协议轴写入当前 frame；未知枚举不产生事实。
                            match axis {
                                WEnum::Value(wl_pointer::Axis::VerticalScroll) => {
                                    pointer_axis_frame
                                        .borrow_mut()
                                        .record_continuous(false, value)
                                }
                                WEnum::Value(wl_pointer::Axis::HorizontalScroll) => {
                                    pointer_axis_frame
                                        .borrow_mut()
                                        .record_continuous(true, value)
                                }
                                _ => return,
                            }
                            // wl_pointer v5 起由 Frame 原子提交；旧版本没有 Frame，立即消费。
                            if pointer.version() < 5 {
                                if let Some((dx, dy)) =
                                    pointer_axis_frame.borrow_mut().take_normalized()
                                {
                                    handle_pointer_axis(
                                        dx,
                                        dy,
                                        &targets,
                                        &pos,
                                        &ev,
                                        &pointer_failures,
                                    );
                                }
                            }
                        }
                        wl_pointer::Event::AxisDiscrete { axis, discrete } => {
                            // v5 离散刻度是传统滚轮的权威单位，在 Frame 时覆盖连续距离。
                            match axis {
                                WEnum::Value(wl_pointer::Axis::VerticalScroll) => {
                                    pointer_axis_frame
                                        .borrow_mut()
                                        .record_discrete(false, discrete)
                                }
                                WEnum::Value(wl_pointer::Axis::HorizontalScroll) => {
                                    pointer_axis_frame
                                        .borrow_mut()
                                        .record_discrete(true, discrete)
                                }
                                _ => {}
                            }
                        }
                        wl_pointer::Event::AxisValue120 { axis, value120 } => {
                            // v8 的 120 基准刻度优先级最高，可保留高分辨率滚轮的分数步长。
                            match axis {
                                WEnum::Value(wl_pointer::Axis::VerticalScroll) => {
                                    pointer_axis_frame
                                        .borrow_mut()
                                        .record_value120(false, value120)
                                }
                                WEnum::Value(wl_pointer::Axis::HorizontalScroll) => {
                                    pointer_axis_frame
                                        .borrow_mut()
                                        .record_value120(true, value120)
                                }
                                _ => {}
                            }
                        }
                        wl_pointer::Event::Frame => {
                            // 同一协议帧的两轴只生成一个平台中立 Wheel 事件。
                            if let Some((dx, dy)) =
                                pointer_axis_frame.borrow_mut().take_normalized()
                            {
                                handle_pointer_axis(dx, dy, &targets, &pos, &ev, &pointer_failures);
                            }
                        }
                        _ => {}
                    });
                    // callback 注册完成后检查式提交唯一 pointer owner。
                    if !install_pointer_proxy(
                        // 提交到 backend 的唯一 pointer 槽。
                        &wl_pointer_handle,
                        // 局部代理失败时由 Component 回滚。
                        ptr,
                        // bind failure 进入同一 backend source。
                        &capability_failures,
                    ) {
                        // 槽损坏时局部代理已释放，停止本次 callback。
                        return;
                    }
                // 仅在 Pointer 能力从有到无时释放代理并推进授权代次。
                } else if pointer_transition == InputProxyTransition::Release {
                    // 三 owner Component 在全部 guards 健康后提交 teardown。
                    if !release_pointer_proxy_checked(
                        // 授权与 generation owner。
                        &pointer_activations,
                        // pointer focus owner。
                        &surface_windows,
                        // 唯一 pointer proxy owner 槽。
                        &wl_pointer_handle,
                        // failure 进入同一 backend source。
                        &capability_failures,
                    ) {
                        // 状态失败时没有半清理，停止本次 capability callback。
                        return;
                    }
                    // pointer 代理成功释放后，旧 Enter serial 不得进入下一代理代次。
                    super::cursor::clear_cursor_pointer_focus(&cursor_state);
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
                    // keyboard callback owner failure 复用 backend pending source。
                    let keyboard_failures = capability_failures.clone();
                    let kbd = seat.get_keyboard();
                    kbd.quick_assign(move |_, event, _| {
                        match event {
                            wl_keyboard::Event::Enter { surface, .. } => {
                                // 五 owner Component 原子提交焦点边沿与输入状态清理。
                                handle_keyboard_enter(
                                    // 转交协议 surface 身份。
                                    surface.id().protocol_id(),
                                    // keyboard focus owner。
                                    &targets,
                                    // Blur/Focus 事件 owner。
                                    &ev,
                                    // 物理按键集合 owner。
                                    &kd,
                                    // 重复候选 owner。
                                    &held_key_info,
                                    // 重复节拍 owner。
                                    &last_repeat_time,
                                    // failure 进入 backend pending source。
                                    &keyboard_failures,
                                );
                            }
                            wl_keyboard::Event::Leave { surface, .. } => {
                                // 五 owner Component 精确匹配 surface 后原子提交 Leave。
                                handle_keyboard_leave(
                                    // 转交协议 surface 身份。
                                    surface.id().protocol_id(),
                                    // keyboard focus owner。
                                    &targets,
                                    // WindowBlur 事件 owner。
                                    &ev,
                                    // 物理按键集合 owner。
                                    &kd,
                                    // 重复候选 owner。
                                    &held_key_info,
                                    // 重复节拍 owner。
                                    &last_repeat_time,
                                    // failure 进入 backend pending source。
                                    &keyboard_failures,
                                );
                            }
                            wl_keyboard::Event::Key {
                                serial, key, state, ..
                            } => {
                                // 先把协议状态收敛为两个明确 Component 端口。
                                match state {
                                    // 已知 Pressed 才允许提交按下事务。
                                    WEnum::Value(wl_keyboard::KeyState::Pressed) => {
                                        // 在 adapter 边界完成 Linux 键码转换。
                                        let code = linux_keycode_to_keycode(key);
                                        // 八 owner Component 原子提交本次物理按下。
                                        handle_keyboard_key_pressed(
                                            // 转交 compositor 签发的真实 serial。
                                            serial,
                                            // 转交平台中立键码。
                                            code,
                                            // keyboard focus 路由 owner。
                                            &targets,
                                            // UI 事件 owner。
                                            &ev,
                                            // 修饰快照 owner。
                                            &mods,
                                            // compositor 重复速率 owner。
                                            &repeat_rate,
                                            // 物理按键集合 owner。
                                            &kd,
                                            // Wayland 输入 serial owner。
                                            &keyboard_serial,
                                            // 客户端重复候选 owner。
                                            &held_key_info,
                                            // 重复节拍 owner。
                                            &last_repeat_time,
                                            // failure 进入 backend pending source。
                                            &keyboard_failures,
                                        );
                                    }
                                    // 已知 Released 才允许提交抬起事务。
                                    WEnum::Value(wl_keyboard::KeyState::Released) => {
                                        // 在 adapter 边界完成 Linux 键码转换。
                                        let code = linux_keycode_to_keycode(key);
                                        // 六 owner Component 原子提交本次物理抬起。
                                        handle_keyboard_key_released(
                                            // 转交平台中立键码。
                                            code,
                                            // keyboard focus 路由 owner。
                                            &targets,
                                            // UI 事件 owner。
                                            &ev,
                                            // 修饰快照 owner。
                                            &mods,
                                            // 物理按键集合 owner。
                                            &kd,
                                            // 客户端重复候选 owner。
                                            &held_key_info,
                                            // 重复节拍 owner。
                                            &last_repeat_time,
                                            // failure 进入 backend pending source。
                                            &keyboard_failures,
                                        );
                                    }
                                    // Wayland 将来新增的已解码状态也不得被解释为 Release。
                                    WEnum::Value(_) => {
                                        // 非 Pressed/Released 值安全丢弃且不修改 owner。
                                    }
                                    // 未知协议枚举不得被误解释为 Release。
                                    WEnum::Unknown(_) => {
                                        // 迟到或未来扩展状态安全丢弃且不修改 owner。
                                    }
                                }
                            }
                            wl_keyboard::Event::Modifiers {
                                mods_depressed,
                                mods_latched,
                                mods_locked,
                                ..
                            } => {
                                // 三 owner Component 原子提交修饰快照与重复状态失效。
                                handle_keyboard_modifiers(
                                    // 转交 depressed 协议位图。
                                    mods_depressed,
                                    // 转交 latched 协议位图。
                                    mods_latched,
                                    // 转交 locked 协议位图。
                                    mods_locked,
                                    // 平台中立修饰快照 owner。
                                    &mods,
                                    // 重复候选 owner。
                                    &held_key_info,
                                    // 重复节拍 owner。
                                    &last_repeat_time,
                                    // failure 进入 backend pending source。
                                    &keyboard_failures,
                                );
                            }
                            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                                // 双 owner Component 原子提交 compositor 重复配置。
                                handle_keyboard_repeat_info(
                                    // 转交原始有符号 repeat rate。
                                    rate,
                                    // 转交原始有符号 repeat delay。
                                    delay,
                                    // 重复速率 owner。
                                    &repeat_rate,
                                    // 重复延迟 owner。
                                    &repeat_delay,
                                    // failure 进入 backend pending source。
                                    &keyboard_failures,
                                );
                            }
                            _ => {}
                        }
                    });
                    // callback 注册完成后检查式提交唯一 keyboard owner。
                    if !install_keyboard_proxy(
                        // 提交到 backend 的唯一 keyboard 槽。
                        &wl_keyboard_handle,
                        // 局部代理失败时由 Component 回滚。
                        kbd,
                        // bind failure 进入同一 backend source。
                        &capability_failures,
                    ) {
                        // 槽损坏时局部代理已释放，停止本次 callback。
                        return;
                    }
                // 仅在 Keyboard 能力从有到无时清理状态并释放代理。
                } else if keyboard_transition == InputProxyTransition::Release {
                    // 六 owner Component 在全部 guards 健康后提交 teardown。
                    if !release_keyboard_proxy_checked(
                        // keyboard focus owner。
                        &surface_windows,
                        // WindowBlur 事件 owner。
                        &ptr_events,
                        // 物理按键集合 owner。
                        &keys_down,
                        // 客户端重复候选 owner。
                        &held_key_info,
                        // 重复节拍 owner。
                        &last_repeat_time,
                        // 唯一 keyboard proxy owner 槽。
                        &wl_keyboard_handle,
                        // failure 进入同一 backend source。
                        &capability_failures,
                    ) {
                        // 状态失败时没有半清理，停止本次 capability callback。
                        return;
                    }
                }
            }
        });

        self.seat = Some(seat);

        // ── 分发 Capabilities 事件 ─────────────────────────
        if !self.dispatch_pending_checked("seat capabilities") {
            return;
        }
        for _ in 0..5 {
            // 每轮先检查唯一 pointer proxy owner 槽。
            let has_pointer = match self.pointer.lock() {
                // 健康 guard 只复制代理是否已经绑定。
                Ok(pointer) => pointer.is_some(),
                // 槽损坏不得伪装为尚未绑定并继续协议分发。
                Err(_) => {
                    // 无 Result 通道的 owner failure 进入 backend source。
                    self.enqueue_failure(Error::new(
                        // poisoned proxy owner 稳定分类为 InvalidState。
                        Errc::InvalidState,
                        // 诊断保留 capability convergence 阶段。
                        "Wayland seat capability pointer proxy slot mutex poisoned during convergence",
                    ));
                    // 不 flush、不 dispatch，也不继续剩余轮次。
                    return;
                }
            };
            // 已绑定时保持既有立即结束收敛语义。
            if has_pointer {
                // 不执行多余 flush/dispatch。
                break;
            }
            // 健康空槽继续既有 flush→dispatch 收敛顺序。
            if !self.flush_checked("seat capabilities")
                || !self.dispatch_pending_checked("seat capabilities")
            {
                // 既有协议 failure 已由 checked helper 处理。
                break;
            }
        }
    }
}
