// Wayland 输入代理 owner Component 不保存协议对象或共享状态。

// pointer/keyboard 槽与 activation registry 使用共享短锁句柄。
use std::sync::{Arc, Mutex};

// 输入代理类型属于 Wayland seat 协议。
use wayland_client::protocol::{wl_keyboard, wl_pointer};
// Proxy trait 提供版本检查与 release 请求。
use wayland_client::Proxy;

// typed failure 保留稳定错误类别与责任边界。
use crate::core::{Errc, Error};
// callback failure 进入 backend 已有 pending source。
use crate::diagnostics::PendingFailureSource;
// surface focus 路由继续由共享 window-target Component 拥有。
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;

// compat Main 是 callback 注册与协议代理的唯一组合 owner。
use super::compat::Main;
// pointer generation 继续由激活注册表唯一拥有。
use super::pointer_activation::WaylandPointerActivationRegistry;

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
