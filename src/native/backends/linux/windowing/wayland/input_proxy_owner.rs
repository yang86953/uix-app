// Wayland 输入代理 owner Component 不保存协议对象或共享状态。

// keyboard Release 事务持有按键集合与事件队列 guards。
use std::collections::{HashSet, VecDeque};
// pointer/keyboard 槽与 activation registry 使用共享短锁句柄。
use std::sync::{Arc, Mutex};
// 私有事务结构显式保存六个 owner 的短期 guards。
use std::sync::MutexGuard;
// 重复节拍 owner 保存单调时刻。
use std::time::Instant;

// 输入代理类型属于 Wayland seat 协议。
use wayland_client::protocol::{wl_keyboard, wl_pointer};
// Proxy trait 提供版本检查与 release 请求。
use wayland_client::Proxy;

// typed failure 保留稳定错误类别与责任边界。
use crate::core::{Errc, Error, WindowId};
// callback failure 进入 backend 已有 pending source。
use crate::diagnostics::PendingFailureSource;
// keyboard Release 在同一事件队列事务中投递 WindowBlur。
use crate::platform::windowing::event::{UiEvent, UiEventPayload, UiEventType};
// keys-down owner 使用统一框架键码。
use crate::platform::windowing::KeyCode;
// surface focus 路由继续由共享 window-target Component 拥有。
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;

// compat Main 是 callback 注册与协议代理的唯一组合 owner。
use super::compat::Main;
// pointer generation 继续由激活注册表唯一拥有。
use super::pointer_activation::WaylandPointerActivationRegistry;
// held-key owner 继续使用 Wayland 后端私有值类型。
use super::HeldKeyInfo;

// 将 capability 绑定状态失败投递到同一 backend source。
fn enqueue_owner_failure(
    // 每个 seat callback 捕获同一 pending source。
    pending_failures: &PendingFailureSource,
    // 诊断文本由具体 owner 端口固定提供。
    message: &'static str,
    // Component 只入队，不执行 report/recovery/user code。
) {
    // 忽略 source 已关闭结果，保持迟到 callback 的 teardown 语义。
    let _ = pending_failures.enqueue(Error::new(
        // poisoned shared owner 统一分类为 InvalidState。
        Errc::InvalidState,
        // 保存稳定阶段诊断。
        message,
    ));
}

// 丢弃尚未提交到 owner 槽的 pointer 代理。
fn discard_pointer_proxy(
    // 局部 Main 暂时独占 callback 与协议代理。
    pointer: Main<wl_pointer::WlPointer>,
    // 清理过程不访问共享代理槽。
) {
    // 先删除兼容层 callback，防止局部代理释放后留下强环。
    pointer.clear_callback();
    // wl_pointer.release 从协议版本三开始可用。
    if pointer.version() >= 3 {
        // 只对支持 release 的代理提交协议请求。
        pointer.release();
    }
}

// 丢弃尚未提交到 owner 槽的 keyboard 代理。
fn discard_keyboard_proxy(
    // 局部 Main 暂时独占 callback 与协议代理。
    keyboard: Main<wl_keyboard::WlKeyboard>,
    // 清理过程不访问共享代理槽。
) {
    // 先删除兼容层 callback，防止局部代理释放后留下强环。
    keyboard.clear_callback();
    // wl_keyboard.release 从协议版本三开始可用。
    if keyboard.version() >= 3 {
        // 只对支持 release 的代理提交协议请求。
        keyboard.release();
    }
}

// 按固定 pointer→keyboard 顺序读取 capability 边沿所需槽快照。
pub(crate) fn snapshot_input_proxy_slots(
    // pointer 槽是 backend 对代理的唯一强 owner。
    pointer_slot: &Arc<Mutex<Option<Main<wl_pointer::WlPointer>>>>,
    // keyboard 槽是 backend 对代理的唯一强 owner。
    keyboard_slot: &Arc<Mutex<Option<Main<wl_keyboard::WlKeyboard>>>>,
    // 任一状态失败进入同一 backend source。
    pending_failures: &PendingFailureSource,
    // None 表示 callback 必须在任何 transition/协议创建前停止。
) -> Option<(bool, bool)> {
    // 先检查 pointer owner，只复制代理存在性。
    let pointer_is_bound = match pointer_slot.lock() {
        // 健康 guard 随分支结束释放。
        Ok(pointer) => pointer.is_some(),
        // 损坏槽不得用于 capability 决策。
        Err(_) => {
            // 入队一次稳定 pointer-slot failure。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 诊断保留 snapshot 阶段。
                "Wayland seat capability pointer proxy slot mutex poisoned during snapshot",
            );
            // 不读取 keyboard 槽，也不创建代理。
            return None;
        }
    };
    // pointer guard 已释放后再检查 keyboard owner。
    let keyboard_is_bound = match keyboard_slot.lock() {
        // 健康 guard 随分支结束释放。
        Ok(keyboard) => keyboard.is_some(),
        // 损坏槽不得用于 capability 决策。
        Err(_) => {
            // 入队一次稳定 keyboard-slot failure。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 诊断保留 snapshot 阶段。
                "Wayland seat capability keyboard proxy slot mutex poisoned during snapshot",
            );
            // 不产生任何 transition 或协议对象。
            return None;
        }
    };
    // 两份健康槽快照同时返回给纯 transition 决策。
    Some((pointer_is_bound, keyboard_is_bound))
}

// 检查式读取新 pointer callback 必须捕获的 activation generation。
pub(crate) fn pointer_generation_checked(
    // generation 继续由激活注册表唯一拥有。
    pointer_activations: &Arc<Mutex<WaylandPointerActivationRegistry>>,
    // 状态失败进入同一 backend source。
    pending_failures: &PendingFailureSource,
    // None 表示不得创建 pointer 代理或 callback。
) -> Option<u64> {
    // registry 损坏时不得读取代次或恢复内部授权状态。
    let generation = match pointer_activations.lock() {
        // 健康 guard 只复制当前 pointer generation。
        Ok(registry) => registry.pointer_generation(),
        // 锁中毒必须显式失败。
        Err(_) => {
            // 入队一次稳定 activation-registry failure。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 诊断保留 pointer Bind 阶段。
                "Wayland seat capability pointer activation registry mutex poisoned during bind",
            );
            // 不创建 pointer 代理或 callback。
            return None;
        }
    };
    // 返回健康 owner 的稳定代次快照。
    Some(generation)
}

// 检查式把已注册 callback 的 pointer 代理提交到唯一 owner 槽。
pub(crate) fn install_pointer_proxy(
    // pointer 槽是 backend 对代理的唯一强 owner。
    pointer_slot: &Arc<Mutex<Option<Main<wl_pointer::WlPointer>>>>,
    // 局部代理在成功前仍由调用方事务拥有。
    pointer: Main<wl_pointer::WlPointer>,
    // 状态失败进入同一 backend source。
    pending_failures: &PendingFailureSource,
    // false 表示局部代理已回滚且 callback 必须停止。
) -> bool {
    // 槽损坏时不得覆盖或读取其中的代理状态。
    let mut slot = match pointer_slot.lock() {
        // 健康 guard 允许唯一 owner 提交。
        Ok(slot) => slot,
        // bind commit 失败必须回滚局部代理。
        Err(_) => {
            // 注销 callback 并按协议版本释放局部代理。
            discard_pointer_proxy(pointer);
            // 入队一次稳定 pointer-slot failure。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 诊断保留 Bind commit 阶段。
                "Wayland seat capability pointer proxy slot mutex poisoned during bind commit",
            );
            // 调用方不得继续处理本次 capability callback。
            return false;
        }
    };
    // 健康槽正式接管 Main 与 callback owner。
    *slot = Some(pointer);
    // pointer Bind 提交成功。
    true
}

// 检查式把已注册 callback 的 keyboard 代理提交到唯一 owner 槽。
pub(crate) fn install_keyboard_proxy(
    // keyboard 槽是 backend 对代理的唯一强 owner。
    keyboard_slot: &Arc<Mutex<Option<Main<wl_keyboard::WlKeyboard>>>>,
    // 局部代理在成功前仍由调用方事务拥有。
    keyboard: Main<wl_keyboard::WlKeyboard>,
    // 状态失败进入同一 backend source。
    pending_failures: &PendingFailureSource,
    // false 表示局部代理已回滚且 callback 必须停止。
) -> bool {
    // 槽损坏时不得覆盖或读取其中的代理状态。
    let mut slot = match keyboard_slot.lock() {
        // 健康 guard 允许唯一 owner 提交。
        Ok(slot) => slot,
        // bind commit 失败必须回滚局部代理。
        Err(_) => {
            // 注销 callback 并按协议版本释放局部代理。
            discard_keyboard_proxy(keyboard);
            // 入队一次稳定 keyboard-slot failure。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 诊断保留 Bind commit 阶段。
                "Wayland seat capability keyboard proxy slot mutex poisoned during bind commit",
            );
            // 调用方不得继续处理本次 capability callback。
            return false;
        }
    };
    // 健康槽正式接管 Main 与 callback owner。
    *slot = Some(keyboard);
    // keyboard Bind 提交成功。
    true
}

// 事务化清理 pointer capability Release 涉及的三个 owner。
pub(crate) fn release_pointer_proxy_checked(
    // activation registry 拥有授权与 pointer generation。
    pointer_activations: &Arc<Mutex<WaylandPointerActivationRegistry>>,
    // surface targets 拥有当前 pointer focus。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // pointer 槽拥有唯一协议代理与 callback。
    pointer_slot: &Arc<Mutex<Option<Main<wl_pointer::WlPointer>>>>,
    // 任一状态失败进入同一 backend source。
    pending_failures: &PendingFailureSource,
    // false 表示没有任何 teardown 状态被修改。
) -> bool {
    // 先取得 activation guard，尚不推进 generation。
    let mut activations = match pointer_activations.lock() {
        // 健康 guard 暂不修改，等待其余 owners。
        Ok(activations) => activations,
        // registry 损坏时立即停止。
        Err(_) => {
            // 入队一次稳定 activation failure。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 诊断保留 pointer Release 阶段。
                "Wayland seat capability pointer activation registry mutex poisoned during release",
            );
            // 不清焦点、不取出代理。
            return false;
        }
    };
    // 再取得 surface-targets guard，保持固定第二锁位。
    let mut targets = match surface_windows.lock() {
        // 两个健康 guards 继续等待代理槽。
        Ok(targets) => targets,
        // surface 路由损坏时保持 activation 不变。
        Err(_) => {
            // 先释放 activation guard，避免跨 owner 入队。
            drop(activations);
            // 入队一次稳定 surface-target failure。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 诊断保留 pointer Release 阶段。
                "Wayland seat capability pointer surface targets mutex poisoned during release",
            );
            // 不推进 generation、不取出代理。
            return false;
        }
    };
    // 最后取得 pointer-slot guard，保持固定第三锁位。
    let mut slot = match pointer_slot.lock() {
        // 三个健康 guards 组成 teardown 事务。
        Ok(slot) => slot,
        // 代理槽损坏时保持前两份状态不变。
        Err(_) => {
            // 先释放 surface-targets guard。
            drop(targets);
            // 再释放 activation guard。
            drop(activations);
            // 入队一次稳定 pointer-slot failure。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 诊断保留 pointer Release commit 阶段。
                "Wayland seat capability pointer proxy slot mutex poisoned during release",
            );
            // 不产生任何半 teardown。
            return false;
        }
    };
    // 三个 owners 均健康后撤销授权并推进 generation。
    activations.invalidate_pointer();
    // 同一事务中清除陈旧 pointer focus。
    targets.clear_pointer_focus();
    // 最后从唯一 owner 槽取出协议代理。
    let pointer = slot.take();
    // 先释放代理槽 guard。
    drop(slot);
    // 再释放 surface-targets guard。
    drop(targets);
    // 最后释放 activation guard。
    drop(activations);
    // 只有实际存在的代理需要在锁外注销和释放。
    if let Some(pointer) = pointer {
        // 复用未提交代理相同的安全清理序列。
        discard_pointer_proxy(pointer);
    }
    // 空槽与实际释放均为幂等成功。
    true
}

// keyboard Release 的六个 guards 只在一次 capability callback 内存活。
struct KeyboardReleaseGuards<'a> {
    // surface 路由 guard 拥有 keyboard focus。
    targets: MutexGuard<'a, SurfaceWindowTargets>,
    // 仅旧焦点存在时持有事件队列 guard。
    events: Option<MutexGuard<'a, VecDeque<UiEvent>>>,
    // keys-down guard 拥有全部物理按键状态。
    keys_down: MutexGuard<'a, HashSet<KeyCode>>,
    // held-key guard 拥有客户端重复候选。
    held_key: MutexGuard<'a, Option<HeldKeyInfo>>,
    // last-time guard 拥有重复节拍状态。
    last_repeat: MutexGuard<'a, Option<Instant>>,
    // slot guard 拥有唯一 keyboard 协议代理。
    slot: MutexGuard<'a, Option<Main<wl_keyboard::WlKeyboard>>>,
    // 旧焦点窗口用于同一事务内的 blur 路由。
    blurred_window: Option<WindowId>,
}

// 按固定顺序取得 keyboard Release 所需全部 owner guards。
fn lock_keyboard_release_owners<'a>(
    // 第一 owner 是 surface keyboard focus。
    surface_windows: &'a Mutex<SurfaceWindowTargets>,
    // 第二 owner 仅在存在旧焦点时需要。
    events: &'a Mutex<VecDeque<UiEvent>>,
    // 第三 owner 是物理按键集合。
    keys_down: &'a Mutex<HashSet<KeyCode>>,
    // 第四 owner 是客户端重复候选。
    held_key_info: &'a Mutex<Option<HeldKeyInfo>>,
    // 第五 owner 是重复节拍。
    last_repeat_time: &'a Mutex<Option<Instant>>,
    // 最后 owner 是唯一 keyboard 代理槽。
    keyboard_slot: &'a Mutex<Option<Main<wl_keyboard::WlKeyboard>>>,
    // 静态错误文本由外层在所有 guards 释放后入队。
) -> std::result::Result<KeyboardReleaseGuards<'a>, &'static str> {
    // 先取得 surface target guard，尚不清除焦点。
    let targets = surface_windows.lock().map_err(|_| {
        // 诊断保留 keyboard Release 的首个 owner。
        "Wayland seat capability keyboard surface targets mutex poisoned during release"
    })?;
    // 在同一健康 guard 下复制旧焦点窗口。
    let blurred_window = targets.keyboard_target();
    // 只有需要投递 blur 时才取得事件队列 owner。
    let event_queue = if blurred_window.is_some() {
        // event queue 损坏时自动释放 surface guard 后返回。
        Some(events.lock().map_err(|_| {
            // 诊断保留可选事件提交阶段。
            "Wayland seat capability keyboard event queue mutex poisoned during release"
        })?)
    } else {
        // 无旧焦点时不需要事件队列租约。
        None
    };
    // 第三步检查 keys-down owner。
    let keys_down = keys_down.lock().map_err(|_| {
        // 诊断保留按键集合清理阶段。
        "Wayland seat capability keyboard keys-down mutex poisoned during release"
    })?;
    // 第四步检查 held-key owner。
    let held_key = held_key_info.lock().map_err(|_| {
        // 诊断保留重复候选清理阶段。
        "Wayland seat capability keyboard held-key mutex poisoned during release"
    })?;
    // 第五步检查 last-time owner。
    let last_repeat = last_repeat_time.lock().map_err(|_| {
        // 诊断保留重复节拍清理阶段。
        "Wayland seat capability keyboard last-time mutex poisoned during release"
    })?;
    // 最后检查 keyboard proxy slot owner。
    let slot = keyboard_slot.lock().map_err(|_| {
        // 诊断保留代理取出提交阶段。
        "Wayland seat capability keyboard proxy slot mutex poisoned during release"
    })?;
    // 全部 owners 健康后才把 guards 交给提交端口。
    Ok(KeyboardReleaseGuards {
        // 保存 surface focus guard。
        targets,
        // 保存可选事件队列 guard。
        events: event_queue,
        // 保存 keys-down guard。
        keys_down,
        // 保存 held-key guard。
        held_key,
        // 保存 last-time guard。
        last_repeat,
        // 保存 keyboard-slot guard。
        slot,
        // 保存旧焦点窗口快照。
        blurred_window,
    })
}

// 事务化清理 keyboard capability Release 涉及的六个 owner。
pub(crate) fn release_keyboard_proxy_checked(
    // surface targets 拥有当前 keyboard focus。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // events 拥有逐窗 WindowBlur 事实。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // keys-down 拥有物理按键集合。
    keys_down: &Arc<Mutex<HashSet<KeyCode>>>,
    // held-key 拥有客户端重复候选。
    held_key_info: &Arc<Mutex<Option<HeldKeyInfo>>>,
    // last-time 拥有重复节拍。
    last_repeat_time: &Arc<Mutex<Option<Instant>>>,
    // keyboard 槽拥有唯一协议代理与 callback。
    keyboard_slot: &Arc<Mutex<Option<Main<wl_keyboard::WlKeyboard>>>>,
    // 任一状态失败进入同一 backend source。
    pending_failures: &PendingFailureSource,
    // false 表示所有状态均保持未修改。
) -> bool {
    // 锁 helper 在失败返回前自动释放已取得的所有 guards。
    let mut owners = match lock_keyboard_release_owners(
        // 固定第一 owner。
        surface_windows,
        // 固定可选第二 owner。
        events,
        // 固定第三 owner。
        keys_down,
        // 固定第四 owner。
        held_key_info,
        // 固定第五 owner。
        last_repeat_time,
        // 固定最后 owner。
        keyboard_slot,
    ) {
        // 全部 guards 健康后进入唯一提交分支。
        Ok(owners) => owners,
        // 任一 lock failure 不产生半 teardown。
        Err(message) => {
            // 此处已不持有任何 owner guard。
            enqueue_owner_failure(pending_failures, message);
            // seat callback 必须停止。
            return false;
        }
    };
    // 全部 owners 健康后首先清除 keyboard focus。
    owners.targets.clear_keyboard_focus();
    // 旧焦点存在时在同一事务中投递一次 WindowBlur。
    if let Some(window_id) = owners.blurred_window {
        // 旧焦点存在保证可选 event guard 已取得。
        let events = owners
            // 借用 guards 结构中的可选队列。
            .events
            // 取得唯一可变队列访问。
            .as_mut()
            // 构造期不变量保证该分支必有 guard。
            .expect("blurred keyboard window requires event queue guard");
        // 投递与既有 seat adapter 相同的逐窗 WindowBlur。
        events.push_back(
            // 构造统一窗口模糊事实。
            UiEvent::new(UiEventType::WindowBlur, UiEventPayload::None)
                // 保留旧焦点窗口路由。
                .for_window(window_id),
        );
    }
    // 在同一事务中清除全部物理按键状态。
    owners.keys_down.clear();
    // 清除客户端重复候选。
    *owners.held_key = None;
    // 清除上次重复节拍。
    *owners.last_repeat = None;
    // 最后从唯一 owner 槽取出 keyboard 代理。
    let keyboard = owners.slot.take();
    // 一次释放六个 guards 后再执行协议 teardown。
    drop(owners);
    // 只有实际存在的代理需要在锁外注销和释放。
    if let Some(keyboard) = keyboard {
        // 复用未提交代理相同的安全清理序列。
        discard_keyboard_proxy(keyboard);
    }
    // 空槽与实际释放均为幂等成功。
    true
}
