// Wayland keyboard focus owner Component 只协调共享状态，不持有协议对象。

// 输入状态与 UI 事件使用标准集合。
use std::collections::{HashSet, VecDeque};
// 多个 callback 通过短时互斥共享 owner 状态。
use std::sync::{Arc, Mutex, MutexGuard};
// 重复节拍 owner 保存单调时刻。
use std::time::Instant;

// typed failure 保留 owner 与焦点阶段。
use crate::core::{Errc, Error, WindowId};
// callback failure 进入 backend 已有 pending source。
use crate::diagnostics::PendingFailureSource;
// 焦点边沿在事务内直接提交到已持有队列。
use crate::native::windowing::event::{UiEvent, UiEventPayload, UiEventType};
// keys-down owner 使用平台中立键码。
use crate::native::windowing::input::KeyCode;
// surface 路由继续由 window-target Component 唯一拥有。
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;

// held-key owner 继续使用 Wayland 后端私有值类型。
use super::HeldKeyInfo;

// 单个焦点事务持有全部共享状态 guards。
struct KeyboardFocusGuards<'a> {
    // 第一 guard 拥有 surface keyboard focus。
    targets: MutexGuard<'a, SurfaceWindowTargets>,
    // 焦点窗口变化时持有 UI 事件队列。
    events: Option<MutexGuard<'a, VecDeque<UiEvent>>>,
    // keys-down guard 拥有全部物理按键状态。
    keys_down: MutexGuard<'a, HashSet<KeyCode>>,
    // held-key guard 拥有客户端重复候选。
    held_key: MutexGuard<'a, Option<HeldKeyInfo>>,
    // last-time guard 拥有重复节拍状态。
    last_repeat: MutexGuard<'a, Option<Instant>>,
}

// 每个焦点事件为四个后续 owners 提供稳定诊断。
#[derive(Clone, Copy)]
struct KeyboardFocusFailureMessages {
    // 可选 event queue 的失败文本。
    events: &'static str,
    // keys-down owner 的失败文本。
    keys_down: &'static str,
    // held-key owner 的失败文本。
    held_key: &'static str,
    // last-repeat owner 的失败文本。
    last_repeat: &'static str,
}

// Enter 使用独立且可定位的 owner 诊断。
const ENTER_FAILURES: KeyboardFocusFailureMessages = KeyboardFocusFailureMessages {
    // Enter 焦点边沿的 event queue 失败。
    events: "Wayland keyboard Enter event queue mutex poisoned",
    // Enter 清理物理键状态失败。
    keys_down: "Wayland keyboard Enter keys-down mutex poisoned",
    // Enter 清理重复候选失败。
    held_key: "Wayland keyboard Enter held-key mutex poisoned",
    // Enter 清理重复节拍失败。
    last_repeat: "Wayland keyboard Enter last-repeat mutex poisoned",
};

// Leave 使用独立且可定位的 owner 诊断。
const LEAVE_FAILURES: KeyboardFocusFailureMessages = KeyboardFocusFailureMessages {
    // Leave 焦点边沿的 event queue 失败。
    events: "Wayland keyboard Leave event queue mutex poisoned",
    // Leave 清理物理键状态失败。
    keys_down: "Wayland keyboard Leave keys-down mutex poisoned",
    // Leave 清理重复候选失败。
    held_key: "Wayland keyboard Leave held-key mutex poisoned",
    // Leave 清理重复节拍失败。
    last_repeat: "Wayland keyboard Leave last-repeat mutex poisoned",
};

// 将 keyboard focus owner failure 投递到同一 backend source。
fn enqueue_owner_failure(
    // 每个 keyboard callback 捕获同一 pending source。
    pending_failures: &PendingFailureSource,
    // 诊断文本由 Enter/Leave 与具体 owner 固定提供。
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

// 在已持有 surface guard 后沿统一顺序取得其余输入状态 owners。
fn lock_keyboard_state_owners<'a>(
    // surface guard 必须由调用方第一步取得。
    targets: MutexGuard<'a, SurfaceWindowTargets>,
    // event queue 是焦点边沿的可选第二 owner。
    events: &'a Mutex<VecDeque<UiEvent>>,
    // keys-down 是固定第三 owner。
    keys_down: &'a Mutex<HashSet<KeyCode>>,
    // held-key 是固定第四 owner。
    held_key_info: &'a Mutex<Option<HeldKeyInfo>>,
    // last-repeat 是固定第五 owner。
    last_repeat_time: &'a Mutex<Option<Instant>>,
    // 只有窗口级焦点事实变化时才需要事件 guard。
    needs_events: bool,
    // 失败诊断由具体 Enter/Leave 端口提供。
    failures: KeyboardFocusFailureMessages,
    // 任一失败自动释放此前 guards 并返回稳定文本。
) -> std::result::Result<KeyboardFocusGuards<'a>, &'static str> {
    // 第二步按需取得 UI 事件队列 owner。
    let events = if needs_events {
        // queue 损坏时不允许先修改 surface focus。
        Some(events.lock().map_err(|_| failures.events)?)
    // 同窗重复 Enter 不产生窗口级焦点事件。
    } else {
        // 无事件事实时跳过无关 owner。
        None
    };
    // 第三步取得 keys-down owner。
    let keys_down = keys_down.lock().map_err(|_| failures.keys_down)?;
    // 第四步取得 held-key owner。
    let held_key = held_key_info.lock().map_err(|_| failures.held_key)?;
    // 第五步取得 last-repeat owner。
    let last_repeat = last_repeat_time
        // 检查重复节拍 owner 健康性。
        .lock()
        // 转换为稳定 Leave/Enter 诊断。
        .map_err(|_| failures.last_repeat)?;
    // 全部所需 owners 健康后返回事务 guards。
    Ok(KeyboardFocusGuards {
        // 保存第一 surface guard。
        targets,
        // 保存可选事件 guard。
        events,
        // 保存物理键 guard。
        keys_down,
        // 保存重复候选 guard。
        held_key,
        // 保存重复节拍 guard。
        last_repeat,
    })
}

// 把 WindowBlur 定向提交到已持有事件队列。
fn push_window_blur(
    // 队列 guard 已由焦点事务独占。
    events: &mut VecDeque<UiEvent>,
    // 旧焦点窗口接收唯一 Blur。
    window_id: WindowId,
    // helper 不访问其他共享 owner。
) {
    // 直接提交与既有 adapter 相同的窗口模糊事实。
    events.push_back(
        // 构造平台中立 WindowBlur。
        UiEvent::new(UiEventType::WindowBlur, UiEventPayload::None)
            // 定向到旧焦点窗口。
            .for_window(window_id),
    );
}

// 把 WindowFocus 定向提交到已持有事件队列。
fn push_window_focus(
    // 队列 guard 已由焦点事务独占。
    events: &mut VecDeque<UiEvent>,
    // 新焦点窗口接收唯一 Focus。
    window_id: WindowId,
    // helper 不访问其他共享 owner。
) {
    // 直接提交与既有 adapter 相同的窗口聚焦事实。
    events.push_back(
        // 构造平台中立 WindowFocus。
        UiEvent::new(UiEventType::WindowFocus, UiEventPayload::None)
            // 定向到新焦点窗口。
            .for_window(window_id),
    );
}

// 在五类 owners 健康后一次提交 keyboard Enter。
pub(crate) fn handle_keyboard_enter(
    // 协议 surface 身份用于解析新窗口焦点。
    surface_id: u32,
    // surface targets 是 keyboard focus 的唯一事实 owner。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // event queue 接收可选 Blur 与 Focus。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // keys-down 在新焦点建立时全部清空。
    keys_down: &Arc<Mutex<HashSet<KeyCode>>>,
    // held-key 在新焦点建立时清空。
    held_key_info: &Arc<Mutex<Option<HeldKeyInfo>>>,
    // last-repeat 在新焦点建立时清空。
    last_repeat_time: &Arc<Mutex<Option<Instant>>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全丢弃当前协议事件。
) {
    // 第一把锁固定为 surface 路由 owner。
    let targets = match surface_windows.lock() {
        // 健康 guard 保持到全部焦点与输入事实提交完成。
        Ok(targets) => targets,
        // 路由损坏时不得推测旧或新窗口身份。
        Err(_) => {
            // 投递可定位的 Enter 路由失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Enter 与 surface owner 阶段。
                "Wayland keyboard Enter surface targets mutex poisoned",
            );
            // 其余 owners 保持原状。
            return;
        }
    };
    // 在尚未修改 focus 前复制旧窗口身份。
    let previous_window = targets.keyboard_target();
    // 在同一 surface guard 下解析待进入窗口。
    let window_id = targets.window_for_surface(surface_id);
    // 仅窗口级焦点变化需要事件队列 owner。
    let needs_events = previous_window != window_id;
    // 沿 Release 权威顺序取得其余四类 owners。
    let mut owners = match lock_keyboard_state_owners(
        // 传入已持有的第一 surface guard。
        targets,
        // 第二 owner 是可选事件队列。
        events,
        // 第三 owner 是 keys-down。
        keys_down,
        // 第四 owner 是 held-key。
        held_key_info,
        // 第五 owner 是 last-repeat。
        last_repeat_time,
        // 传入窗口级边沿判定。
        needs_events,
        // 使用 Enter 专用诊断。
        ENTER_FAILURES,
    ) {
        // 全部 guards 健康后进入唯一提交分支。
        Ok(owners) => owners,
        // 任一 owner failure 不产生半焦点切换。
        Err(message) => {
            // 此处已自动释放此前取得的 guards。
            enqueue_owner_failure(pending_failures, message);
            // 安全丢弃当前 Enter。
            return;
        }
    };
    // 所有 guards 健康后提交精确 surface keyboard focus。
    owners.targets.keyboard_enter(surface_id);
    // 窗口级焦点变化时提交旧 Blur 与新 Focus。
    if needs_events {
        // 构造期保证该分支持有事件队列 guard。
        let event_queue = owners
            // 借用可选事件队列。
            .events
            // 取得唯一可变队列访问。
            .as_mut()
            // needs_events 与 guard 存在性由同一次构造保证。
            .expect("keyboard Enter focus edge requires event queue guard");
        // 旧焦点存在时先投递 WindowBlur。
        if let Some(previous_window) = previous_window {
            // 定向到 Enter 前的旧窗口。
            push_window_blur(event_queue, previous_window);
        }
        // 新 surface 已注册时随后投递 WindowFocus。
        if let Some(window_id) = window_id {
            // 定向到 Enter 解析出的新窗口。
            push_window_focus(event_queue, window_id);
        }
    }
    // 同一事务清除全部物理按键状态。
    owners.keys_down.clear();
    // 清除客户端重复候选。
    *owners.held_key = None;
    // 清除上次重复节拍。
    *owners.last_repeat = None;
}

// 在五类 owners 健康后一次提交精确 keyboard Leave。
pub(crate) fn handle_keyboard_leave(
    // 协议 surface 身份阻止迟到 Leave 清除新焦点。
    surface_id: u32,
    // surface targets 是 keyboard focus 的唯一事实 owner。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // event queue 接收唯一 WindowBlur。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // keys-down 在焦点离开时全部清空。
    keys_down: &Arc<Mutex<HashSet<KeyCode>>>,
    // held-key 在焦点离开时清空。
    held_key_info: &Arc<Mutex<Option<HeldKeyInfo>>>,
    // last-repeat 在焦点离开时清空。
    last_repeat_time: &Arc<Mutex<Option<Instant>>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全保留原状态。
) {
    // 第一把锁固定为 surface 路由 owner。
    let targets = match surface_windows.lock() {
        // 健康 guard 允许精确比较焦点 surface。
        Ok(targets) => targets,
        // 路由损坏时不得推测旧窗口身份。
        Err(_) => {
            // 投递可定位的 Leave 路由失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Leave 与 surface owner 阶段。
                "Wayland keyboard Leave surface targets mutex poisoned",
            );
            // 其余 owners 保持原状。
            return;
        }
    };
    // 同一不可变焦点快照必须匹配本次 Leave surface。
    let Some((focused_surface, window_id)) = targets.keyboard_target_identity() else {
        // 无焦点 Leave 保持幂等。
        return;
    };
    // 迟到的其他 surface Leave 不得清除当前焦点。
    if focused_surface != surface_id {
        // 保留新 surface 的全部输入状态。
        return;
    }
    // 沿 Release 权威顺序取得事件与三类输入状态 owners。
    let mut owners = match lock_keyboard_state_owners(
        // 传入已持有的第一 surface guard。
        targets,
        // 匹配焦点必然需要 WindowBlur 队列。
        events,
        // 第三 owner 是 keys-down。
        keys_down,
        // 第四 owner 是 held-key。
        held_key_info,
        // 第五 owner 是 last-repeat。
        last_repeat_time,
        // 精确焦点 Leave 必须投递 Blur。
        true,
        // 使用 Leave 专用诊断。
        LEAVE_FAILURES,
    ) {
        // 全部 guards 健康后进入唯一提交分支。
        Ok(owners) => owners,
        // 任一 owner failure 不产生半焦点清理。
        Err(message) => {
            // 此处已自动释放此前取得的 guards。
            enqueue_owner_failure(pending_failures, message);
            // 安全保留原焦点与输入状态。
            return;
        }
    };
    // 所有 guards 健康后清除精确 surface focus。
    owners.targets.keyboard_leave(surface_id);
    // 构造期保证匹配 Leave 持有事件队列 guard。
    let event_queue = owners
        // 借用可选事件队列。
        .events
        // 取得唯一可变队列访问。
        .as_mut()
        // 精确焦点 Leave 与 guard 存在性由同一次构造保证。
        .expect("keyboard Leave focus edge requires event queue guard");
    // 同一事务向旧焦点窗口投递 WindowBlur。
    push_window_blur(event_queue, window_id);
    // 清除全部物理按键状态。
    owners.keys_down.clear();
    // 清除客户端重复候选。
    *owners.held_key = None;
    // 清除上次重复节拍。
    *owners.last_repeat = None;
}
