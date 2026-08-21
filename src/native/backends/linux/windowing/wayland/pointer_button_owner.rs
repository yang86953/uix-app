// Wayland pointer Button owner Component 只协调共享状态，不持有协议对象。

// UI 事件 owner 使用先进先出队列。
use std::collections::VecDeque;
// 多个 callback 通过短时互斥共享 owner 状态。
use std::sync::{Arc, Mutex};

// typed failure 保留 owner 与 Button 阶段。
use crate::core::{Errc, Error};
// callback failure 进入 backend 已有 pending source。
use crate::diagnostics::PendingFailureSource;
// PointerDown/Up 在事务内直接提交到已持有队列。
use crate::platform::windowing::event::UiEvent;
// 平台中立按钮由 seat adapter 完成 Linux 编号映射。
use crate::native::windowing::input::MouseButton;
// 输入 serial owner 继续服务剪贴板等 Wayland 授权操作。
use crate::native::windowing::shared::input_serial::InputSerial;
// surface 路由继续由 window-target Component 唯一拥有。
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;

// 最近指针位置继续由 Wayland backend owner 保存。
use super::LastPointerState;
// 指针激活注册表独占原生窗口拖动授权生命周期。
use super::pointer_activation::WaylandPointerActivationRegistry;

// 将 Button callback owner failure 投递到同一 backend source。
fn enqueue_owner_failure(
    // 每个 pointer callback 捕获同一 pending source。
    pending_failures: &PendingFailureSource,
    // 诊断文本由 Press/Release 与具体 owner 固定提供。
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

// 在全部相关 owners 健康后一次提交 pointer Button Press。
pub(crate) fn handle_pointer_button_pressed(
    // compositor 为本次按下签发的 raw serial 只留在 Wayland 私有层。
    serial: u32,
    // 平台中立按钮保留未知编号的 MouseButton::None 语义。
    button: MouseButton,
    // callback 捕获的 pointer 代次约束主键授权。
    pointer_generation: u64,
    // activation registry 是主键路径全局锁序的第一 owner。
    pointer_activations: &Arc<Mutex<WaylandPointerActivationRegistry>>,
    // surface targets 提供按下时的精确焦点身份。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // last pointer 提供按下时的精确 surface-local 坐标。
    last_pointer: &Arc<Mutex<LastPointerState>>,
    // UI 事件队列接收同一事务中的 PointerDown。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // input serial 与事件保持同一健康快照后提交。
    input_serial: &Arc<Mutex<InputSerial>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全丢弃当前协议事件。
) {
    // 只有主键路径需要获取 activation registry。
    let mut activations = if button == MouseButton::Left {
        // 第一把锁沿用 surface 注册与 capability Release 的全局顺序。
        match pointer_activations.lock() {
            // 健康 guard 保持到授权、serial 与事件提交完成。
            Ok(activations) => Some(activations),
            // 注册表损坏时不得只记录 serial 或投递 PointerDown。
            Err(_) => {
                // 投递可定位的 Press 授权失败。
                enqueue_owner_failure(
                    // 使用同一 backend source。
                    pending_failures,
                    // 保留 Press 与 activation owner 阶段。
                    "Wayland pointer Button Press activation registry mutex poisoned",
                );
                // 五份共享事实保持原状。
                return;
            }
        }
    // 非主键不依赖窗口拖动授权 owner。
    } else {
        // 跳过无关 registry，避免其损坏阻断普通按钮输入。
        None
    };
    // 主键的第二把、非主键的第一把锁固定为 surface 路由。
    let targets = match surface_windows.lock() {
        // 健康 guard 保持到事件提交完成。
        Ok(targets) => targets,
        // 路由损坏时不得推测窗口或 surface 身份。
        Err(_) => {
            // 投递可定位的 Press 路由失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Press 与 surface owner 阶段。
                "Wayland pointer Button Press surface targets mutex poisoned",
            );
            // 已持有 activation guard 尚未修改状态。
            return;
        }
    };
    // 无焦点 Press 不签发授权、不记录 serial、不伪造 UI 事件。
    let Some((surface_id, window_id)) = targets.pointer_target_identity() else {
        // 安全丢弃无法定向的协议事件。
        return;
    };
    // 下一把锁固定为最近指针位置 owner。
    let last_pointer = match last_pointer.lock() {
        // 健康 guard 保持到事件提交完成。
        Ok(last_pointer) => last_pointer,
        // 位置损坏时不得伪造默认点击坐标。
        Err(_) => {
            // 投递可定位的 Press 位置失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Press 与 position owner 阶段。
                "Wayland pointer Button Press position mutex poisoned",
            );
            // 授权、serial 与事件均未修改。
            return;
        }
    };
    // 下一把锁固定为 UI 事件队列 owner。
    let mut events = match events.lock() {
        // 健康 guard 保持到 serial owner 检查完成。
        Ok(events) => events,
        // 队列损坏时不得先签发授权或记录 serial。
        Err(_) => {
            // 投递可定位的 Press 队列失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Press 与 event owner 阶段。
                "Wayland pointer Button Press event queue mutex poisoned",
            );
            // 已持有 owners 均未被修改。
            return;
        }
    };
    // 最后一把锁沿用 keyboard Key 的 events→serial 既有顺序。
    let mut input_serial = match input_serial.lock() {
        // 健康 guard 允许开始一次性共享事实提交。
        Ok(input_serial) => input_serial,
        // serial 损坏时不得只投递事件或签发拖动授权。
        Err(_) => {
            // 投递可定位的 Press serial 失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Press 与 serial owner 阶段。
                "Wayland pointer Button Press input serial mutex poisoned",
            );
            // 前四份 owner 仍未被修改。
            return;
        }
    };
    // 从健康 position owner 复制真实点击坐标。
    let position = last_pointer.position;
    // 先构造不携带平台授权细节的普通 PointerDown。
    let pointer_event = UiEvent::pointer_down(position, button);
    // 主键在所有 guards 健康后尝试签发一次性拖动授权。
    let pointer_event = if let Some(activations) = activations.as_mut() {
        // 使用同一焦点快照签发不可解释激活身份。
        let activation = activations.issue_primary_press(
            // 使用 callback 捕获的 pointer 代次。
            pointer_generation,
            // 绑定按下时的原生 surface 身份。
            surface_id,
            // 绑定按下时的稳定窗口身份。
            window_id,
            // 绑定 compositor 实际签发的 raw serial。
            serial,
        );
        // 只有注册表接受完整身份时才附着授权。
        match activation {
            // 不透明身份随同一个 native 事件交付。
            Some(activation) => pointer_event.with_pointer_activation(activation),
            // 代次或 surface 不匹配时仍保留普通按下语义。
            None => pointer_event,
        }
    // 非主键永远不附着窗口拖动授权。
    } else {
        // 原样保留普通 PointerDown。
        pointer_event
    };
    // 与 PointerDown 同一事务记录最近合法输入 serial。
    input_serial.record(serial);
    // 最后把事件定向到同一焦点快照中的窗口。
    events.push_back(pointer_event.for_window(window_id));
}

// 在全部相关 owners 健康后一次提交 pointer Button Release。
pub(crate) fn handle_pointer_button_released(
    // 平台中立按钮保留未知编号的 MouseButton::None 语义。
    button: MouseButton,
    // callback 捕获的 pointer 代次约束主键授权撤销。
    pointer_generation: u64,
    // activation registry 是主键路径全局锁序的第一 owner。
    pointer_activations: &Arc<Mutex<WaylandPointerActivationRegistry>>,
    // surface targets 提供抬起时的精确焦点身份。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // last pointer 提供抬起时的精确 surface-local 坐标。
    last_pointer: &Arc<Mutex<LastPointerState>>,
    // UI 事件队列接收同一事务中的 PointerUp。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全丢弃当前协议事件。
) {
    // 只有主键路径需要获取 activation registry。
    let mut activations = if button == MouseButton::Left {
        // 第一把锁沿用全局 activation→surface 顺序。
        match pointer_activations.lock() {
            // 健康 guard 保持到授权撤销与事件提交完成。
            Ok(activations) => Some(activations),
            // 注册表损坏时不得只投递 PointerUp。
            Err(_) => {
                // 投递可定位的 Release 授权失败。
                enqueue_owner_failure(
                    // 使用同一 backend source。
                    pending_failures,
                    // 保留 Release 与 activation owner 阶段。
                    "Wayland pointer Button Release activation registry mutex poisoned",
                );
                // 授权与事件均保持原状。
                return;
            }
        }
    // 非主键不依赖窗口拖动授权 owner。
    } else {
        // 跳过无关 registry，保持普通按钮可用。
        None
    };
    // 主键的第二把、非主键的第一把锁固定为 surface 路由。
    let targets = match surface_windows.lock() {
        // 健康 guard 保持到事件提交完成。
        Ok(targets) => targets,
        // 路由损坏时不得推测窗口身份。
        Err(_) => {
            // 投递可定位的 Release 路由失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Release 与 surface owner 阶段。
                "Wayland pointer Button Release surface targets mutex poisoned",
            );
            // 已持有 activation guard 尚未修改状态。
            return;
        }
    };
    // 无焦点 Release 不撤销其他窗口授权、不伪造 UI 事件。
    let Some((_, window_id)) = targets.pointer_target_identity() else {
        // 安全丢弃无法定向的协议事件。
        return;
    };
    // 下一把锁固定为最近指针位置 owner。
    let last_pointer = match last_pointer.lock() {
        // 健康 guard 保持到事件提交完成。
        Ok(last_pointer) => last_pointer,
        // 位置损坏时不得伪造默认抬起坐标。
        Err(_) => {
            // 投递可定位的 Release 位置失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Release 与 position owner 阶段。
                "Wayland pointer Button Release position mutex poisoned",
            );
            // 授权与事件均未修改。
            return;
        }
    };
    // 最后一把锁固定为 UI 事件队列 owner。
    let mut events = match events.lock() {
        // 健康 guard 允许开始一次性提交。
        Ok(events) => events,
        // 队列损坏时不得先撤销授权。
        Err(_) => {
            // 投递可定位的 Release 队列失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Release 与 event owner 阶段。
                "Wayland pointer Button Release event queue mutex poisoned",
            );
            // 已持有 owners 均未被修改。
            return;
        }
    };
    // 从健康 position owner 复制真实抬起坐标。
    let position = last_pointer.position;
    // 主键在全部 guards 健康后撤销同一 pointer 代次授权。
    if let Some(activations) = activations.as_mut() {
        // 旧代理 Release 不得影响新代次授权。
        activations.revoke_primary_press(pointer_generation);
    }
    // 同一事务把 PointerUp 定向到抬起时的焦点窗口。
    events.push_back(UiEvent::pointer_up(position, button).for_window(window_id));
}
