// Wayland pointer focus owner Component 只协调共享状态，不持有协议对象。

// UI 事件 owner 使用先进先出队列。
use std::collections::VecDeque;
// 多个 callback 通过短时互斥共享 owner 状态。
use std::sync::{Arc, Mutex};

// typed failure 保留 owner 与 callback 阶段。
use crate::core::{Errc, Error, Point};
// callback failure 进入 backend 已有 pending source。
use crate::diagnostics::PendingFailureSource;
// PointerMove 事件在事务内直接提交到已持有队列。
use crate::platform::windowing::event::UiEvent;
// Wheel 事件使用平台中立的空键盘修饰快照。
use crate::native::windowing::input::KeyMod;
// surface 路由继续由 window-target Component 唯一拥有。
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;

// 最近指针位置继续由 Wayland backend owner 保存。
use super::LastPointerState;
// 指针激活注册表独占原生拖动授权生命周期。
use super::pointer_activation::WaylandPointerActivationRegistry;

// 将 pointer callback owner failure 投递到同一 backend source。
fn enqueue_owner_failure(
    // 每个 pointer callback 捕获同一 pending source。
    pending_failures: &PendingFailureSource,
    // 诊断文本由具体事件与 owner 固定提供。
    message: &'static str,
    // Component 只入队，不执行最终错误策略。
) {
    // source 已关闭时保持迟到 callback 的安全终止语义。
    let _ = pending_failures.enqueue(Error::new(
        // poisoned shared owner 统一分类为 InvalidState。
        Errc::InvalidState,
        // 保存稳定阶段诊断。
        message,
    ));
}

// 在三个 owners 健康后一次提交 pointer Enter 的焦点、位置与事件。
pub(crate) fn handle_pointer_enter(
    // 协议 surface 身份用于解析稳定窗口路由。
    surface_id: u32,
    // surface-local 坐标由 adapter 完成数值转换。
    position: Point,
    // surface targets 是 pointer focus 的唯一事实 owner。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // last pointer 保存后续 Button/Axis 使用的坐标。
    last_pointer: &Arc<Mutex<LastPointerState>>,
    // UI 事件队列接收与焦点同事务的 PointerMove。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全丢弃当前协议事件。
) {
    // 第一把锁固定为 surface 路由 owner。
    let mut targets = match surface_windows.lock() {
        // 健康 guard 保持到三 owner 提交完成。
        Ok(targets) => targets,
        // 路由损坏时不得读取或恢复焦点状态。
        Err(_) => {
            // 投递可定位的 Enter 路由失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留事件与 owner 阶段。
                "Wayland pointer Enter surface targets mutex poisoned",
            );
            // 不修改位置或事件队列。
            return;
        }
    };
    // 在尚未修改焦点前解析注册 surface。
    let Some(window_id) = targets.window_for_surface(surface_id) else {
        // 未知 surface 必须清除旧焦点，且不需要其他 owner。
        targets.pointer_enter(surface_id);
        // 不为未知窗口伪造位置或 UI 事件。
        return;
    };
    // 第二把锁固定为最近指针位置 owner。
    let mut last_pointer = match last_pointer.lock() {
        // 健康 guard 与 targets 一起保留到提交完成。
        Ok(last_pointer) => last_pointer,
        // 位置损坏时不得先提交新焦点。
        Err(_) => {
            // 投递可定位的 Enter 位置失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留事件与 owner 阶段。
                "Wayland pointer Enter position mutex poisoned",
            );
            // 三份共享事实保持原状。
            return;
        }
    };
    // 第三把锁固定为 UI 事件队列 owner。
    let mut events = match events.lock() {
        // 健康 guard 允许开始一次性提交。
        Ok(events) => events,
        // 队列损坏时不得留下无事件的新焦点或坐标。
        Err(_) => {
            // 投递可定位的 Enter 队列失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留事件与 owner 阶段。
                "Wayland pointer Enter event queue mutex poisoned",
            );
            // 前两份 owner 仍未被修改。
            return;
        }
    };
    // 所有 guards 健康后提交新 pointer focus。
    targets.pointer_enter(surface_id);
    // 同一事务更新后续按钮与滚轮使用的位置。
    last_pointer.position = position;
    // 同一事务把 PointerMove 定向到已解析窗口。
    events.push_back(UiEvent::pointer_move(position).for_window(window_id));
}

// 在三个 owners 健康后一次提交 pointer Motion 的位置与事件。
pub(crate) fn handle_pointer_motion(
    // surface-local 坐标由 adapter 完成数值转换。
    position: Point,
    // surface targets 提供当前稳定 pointer focus。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // last pointer 保存后续 Button/Axis 使用的坐标。
    last_pointer: &Arc<Mutex<LastPointerState>>,
    // UI 事件队列接收与位置同事务的 PointerMove。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全丢弃当前协议事件。
) {
    // 第一把锁固定为 surface 路由 owner。
    let targets = match surface_windows.lock() {
        // 健康 guard 保持到位置与事件提交完成。
        Ok(targets) => targets,
        // 路由损坏时不得推测旧窗口身份。
        Err(_) => {
            // 投递可定位的 Motion 路由失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留事件与 owner 阶段。
                "Wayland pointer Motion surface targets mutex poisoned",
            );
            // 不修改位置或事件队列。
            return;
        }
    };
    // 无焦点 Motion 保持既有安全丢弃语义。
    let Some(window_id) = targets.pointer_target() else {
        // 不为无目标事件获取其余 owners。
        return;
    };
    // 第二把锁固定为最近指针位置 owner。
    let mut last_pointer = match last_pointer.lock() {
        // 健康 guard 与 targets 一起保留到提交完成。
        Ok(last_pointer) => last_pointer,
        // 位置损坏时不得只投递移动事件。
        Err(_) => {
            // 投递可定位的 Motion 位置失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留事件与 owner 阶段。
                "Wayland pointer Motion position mutex poisoned",
            );
            // 位置与事件事实保持原状。
            return;
        }
    };
    // 第三把锁固定为 UI 事件队列 owner。
    let mut events = match events.lock() {
        // 健康 guard 允许开始一次性提交。
        Ok(events) => events,
        // 队列损坏时不得先提交新坐标。
        Err(_) => {
            // 投递可定位的 Motion 队列失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留事件与 owner 阶段。
                "Wayland pointer Motion event queue mutex poisoned",
            );
            // 已持有 owners 尚未被修改。
            return;
        }
    };
    // 所有 guards 健康后提交最近位置。
    last_pointer.position = position;
    // 同一事务把 PointerMove 定向到当前焦点窗口。
    events.push_back(UiEvent::pointer_move(position).for_window(window_id));
}

// 在三个 owners 健康后一次提交 pointer Axis 的定向滚轮事件。
pub(crate) fn handle_pointer_axis(
    // 水平滚轮增量已由 adapter 从协议枚举映射。
    delta_x: f32,
    // 垂直滚轮增量已由 adapter 从协议枚举映射。
    delta_y: f32,
    // surface targets 提供当前稳定 pointer focus。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // last pointer 提供滚轮事件发生时的精确 surface-local 坐标。
    last_pointer: &Arc<Mutex<LastPointerState>>,
    // UI 事件队列接收与路由快照同事务的 Wheel。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全丢弃当前协议事件。
) {
    // 未知 Axis 与精确零增量不需要访问任何共享 owner。
    if delta_x == 0.0 && delta_y == 0.0 {
        // 不伪造无效 Wheel 事件。
        return;
    }
    // 第一把锁固定为 surface 路由 owner。
    let targets = match surface_windows.lock() {
        // 健康 guard 保持到滚轮事件提交完成。
        Ok(targets) => targets,
        // 路由损坏时不得推测旧窗口身份。
        Err(_) => {
            // 投递可定位的 Axis 路由失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留事件与 owner 阶段。
                "Wayland pointer Axis surface targets mutex poisoned",
            );
            // 不读取位置或修改事件队列。
            return;
        }
    };
    // 无焦点 Axis 保持既有安全丢弃语义。
    let Some(window_id) = targets.pointer_target() else {
        // 不为无目标事件获取其余 owners。
        return;
    };
    // 第二把锁固定为最近指针位置 owner。
    let last_pointer = match last_pointer.lock() {
        // 健康 guard 与 targets 一起保留到提交完成。
        Ok(last_pointer) => last_pointer,
        // 位置损坏时不得伪造默认坐标。
        Err(_) => {
            // 投递可定位的 Axis 位置失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留事件与 owner 阶段。
                "Wayland pointer Axis position mutex poisoned",
            );
            // 不投递坐标不可信的 Wheel 事件。
            return;
        }
    };
    // 第三把锁固定为 UI 事件队列 owner。
    let mut events = match events.lock() {
        // 健康 guard 允许开始唯一事件提交。
        Ok(events) => events,
        // 队列损坏时不得恢复访问 poisoned 容器。
        Err(_) => {
            // 投递可定位的 Axis 队列失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留事件与 owner 阶段。
                "Wayland pointer Axis event queue mutex poisoned",
            );
            // 前两份 owner 本就只读且没有部分提交。
            return;
        }
    };
    // 从健康 position owner 复制事件坐标。
    let position = last_pointer.position;
    // 所有 guards 健康后一次提交定向 Wheel 事件。
    events.push_back(
        // 构造平台中立滚轮负载。
        UiEvent::wheel(
            // 使用真实最近 surface-local 坐标。
            position,
            // 传入已映射的水平增量。
            delta_x,
            // 传入已映射的垂直增量。
            delta_y,
            // Wayland Axis 事件本身不携带修饰键快照。
            KeyMod::NONE,
        )
        // 将事件绑定到同一焦点快照中的窗口。
        .for_window(window_id),
    );
}

// 按全局 activation→surface 顺序事务化提交精确 pointer Leave。
pub(crate) fn handle_pointer_leave(
    // 协议 surface 身份阻止迟到 Leave 清除新焦点。
    surface_id: u32,
    // callback 捕获的 pointer 代次阻止旧代理撤销新授权。
    pointer_generation: u64,
    // activation registry 必须先于 surface targets 获取。
    pointer_activations: &Arc<Mutex<WaylandPointerActivationRegistry>>,
    // surface targets 是 pointer focus 的唯一事实 owner。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全保留原状态。
) {
    // 第一把锁沿用 capability Release 的全局 activation 顺序。
    let mut activations = match pointer_activations.lock() {
        // 健康 guard 保持到 focus 与授权提交完成。
        Ok(activations) => activations,
        // 注册表损坏时不得只清除 surface focus。
        Err(_) => {
            // 投递可定位的 Leave 授权失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留事件与 owner 阶段。
                "Wayland pointer Leave activation registry mutex poisoned",
            );
            // 两份共享事实保持原状。
            return;
        }
    };
    // 第二把锁固定为 surface 路由 owner。
    let mut targets = match surface_windows.lock() {
        // 健康 guard 允许检查精确焦点身份。
        Ok(targets) => targets,
        // 路由损坏时不得先撤销授权。
        Err(_) => {
            // 投递可定位的 Leave 路由失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留事件与 owner 阶段。
                "Wayland pointer Leave surface targets mutex poisoned",
            );
            // activation guard 尚未修改任何状态。
            return;
        }
    };
    // 同一不可变焦点快照必须匹配本次 Leave surface。
    let Some((focused_surface, window_id)) = targets.pointer_target_identity() else {
        // 无焦点 Leave 保持幂等。
        return;
    };
    // 迟到的其他 surface Leave 不得撤销当前焦点或授权。
    if focused_surface != surface_id {
        // 保留新 surface 的全部状态。
        return;
    }
    // 先使待消费拖动授权失效，保持安全优先的提交顺序。
    activations.revoke_pointer_focus(
        // 使用创建 callback 时捕获的 pointer 代次。
        pointer_generation,
        // 使用精确匹配的协议 surface 身份。
        surface_id,
        // 使用焦点快照中的稳定窗口身份。
        window_id,
    );
    // 随后清除同一 surface 的 pointer focus。
    targets.pointer_leave(surface_id);
}
