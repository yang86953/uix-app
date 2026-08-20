// Wayland keyboard Key owner Component 只协调共享状态，不持有协议对象。

// UI 事件与物理按键 owner 使用标准集合。
use std::collections::{HashSet, VecDeque};
// 多个 callback 通过固定顺序的短时互斥共享状态。
use std::sync::{Arc, Mutex, MutexGuard};
// 客户端重复候选保存首次物理按下时刻。
use std::time::Instant;

// typed failure 保留 Key 阶段与具体 owner。
use crate::core::{Errc, Error, WindowId};
// callback failure 进入 backend 已有 pending source。
use crate::diagnostics::PendingFailureSource;
// KeyDown、KeyUp 与 TextInput 在事务内直接提交到已持有队列。
use crate::native::windowing::event::UiEvent;
// Component 只消费平台中立键码与修饰快照。
use crate::native::windowing::input::{KeyCode, KeyMod};
// 输入 serial owner 继续服务剪贴板等 Wayland 授权操作。
use crate::native::windowing::shared::input_serial::InputSerial;
// surface 路由继续由 window-target Component 唯一拥有。
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;

// ASCII 文本转换继续复用 Wayland 键码适配规则。
use super::keycode::keycode_to_char;
// held-key owner 继续使用 Wayland 后端私有值类型。
use super::HeldKeyInfo;

// Press 事务持有全部八类共享 owner guards。
struct KeyboardKeyPressGuards<'a> {
    // surface guard 保持目标窗口在提交期间稳定。
    _targets: MutexGuard<'a, SurfaceWindowTargets>,
    // event queue guard 接收 KeyDown 与可选 TextInput。
    events: MutexGuard<'a, VecDeque<UiEvent>>,
    // modifiers guard 提供同一按下时刻的修饰快照。
    modifiers: MutexGuard<'a, KeyMod>,
    // repeat-rate guard 决定是否去重 compositor 重复事件。
    repeat_rate: MutexGuard<'a, i32>,
    // keys-down guard 拥有物理按键集合。
    keys_down: MutexGuard<'a, HashSet<KeyCode>>,
    // input-serial guard 记录成功按下对应的协议 serial。
    input_serial: MutexGuard<'a, InputSerial>,
    // held-key guard 拥有客户端重复候选。
    held_key: MutexGuard<'a, Option<HeldKeyInfo>>,
    // last-repeat guard 拥有重复节拍状态。
    last_repeat: MutexGuard<'a, Option<Instant>>,
}

// Release 事务只持有抬起路径所需的六类共享 owner guards。
struct KeyboardKeyReleaseGuards<'a> {
    // surface guard 保持目标窗口在提交期间稳定。
    _targets: MutexGuard<'a, SurfaceWindowTargets>,
    // event queue guard 接收 KeyUp。
    events: MutexGuard<'a, VecDeque<UiEvent>>,
    // modifiers guard 提供同一抬起时刻的修饰快照。
    modifiers: MutexGuard<'a, KeyMod>,
    // keys-down guard 拥有物理按键集合。
    keys_down: MutexGuard<'a, HashSet<KeyCode>>,
    // held-key guard 拥有客户端重复候选。
    held_key: MutexGuard<'a, Option<HeldKeyInfo>>,
    // last-repeat guard 拥有重复节拍状态。
    last_repeat: MutexGuard<'a, Option<Instant>>,
}

// 将 Key callback owner failure 投递到同一 backend source。
fn enqueue_owner_failure(
    // 每个 keyboard callback 捕获同一 pending source。
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

// 按全局顺序取得 Press 所需的八类 owners。
fn lock_press_owners<'a>(
    // surface targets 是 Press 锁序的第一 owner。
    surface_windows: &'a Arc<Mutex<SurfaceWindowTargets>>,
    // event queue 是第二 owner。
    events: &'a Arc<Mutex<VecDeque<UiEvent>>>,
    // modifiers 是第三 owner。
    modifiers: &'a Arc<Mutex<KeyMod>>,
    // repeat-rate 是第四 owner。
    repeat_rate: &'a Arc<Mutex<i32>>,
    // keys-down 是第五 owner。
    keys_down: &'a Arc<Mutex<HashSet<KeyCode>>>,
    // input-serial 是第六 owner。
    input_serial: &'a Arc<Mutex<InputSerial>>,
    // held-key 是第七 owner。
    held_key_info: &'a Arc<Mutex<Option<HeldKeyInfo>>>,
    // last-repeat 是第八 owner。
    last_repeat_time: &'a Arc<Mutex<Option<Instant>>>,
    // 无目标窗口返回 Ok(None)，状态失败返回稳定诊断。
) -> std::result::Result<Option<(WindowId, KeyboardKeyPressGuards<'a>)>, &'static str> {
    // 第一把锁固定为 surface 路由 owner。
    let targets = surface_windows
        // 取得不可恢复的健康 guard。
        .lock()
        // 路由损坏时不得推测目标窗口。
        .map_err(|_| "Wayland keyboard Key Press surface targets mutex poisoned")?;
    // 无 keyboard focus 的 Press 不能定向，保持所有 owner 不变。
    let Some(window_id) = targets.keyboard_target() else {
        // 无目标是可安全丢弃的协议事件，不形成 failure。
        return Ok(None);
    };
    // 第二把锁固定为 UI 事件队列 owner。
    let events = events
        // 在 surface guard 后取得队列。
        .lock()
        // 队列损坏时不得继续写入其他 owner。
        .map_err(|_| "Wayland keyboard Key Press event queue mutex poisoned")?;
    // 第三把锁固定为修饰快照 owner。
    let modifiers = modifiers
        // 在事件队列后取得真实修饰状态。
        .lock()
        // 禁止用 KeyMod::NONE 伪造损坏快照。
        .map_err(|_| "Wayland keyboard Key Press modifiers mutex poisoned")?;
    // 第四把锁固定为 repeat-rate owner。
    let repeat_rate = repeat_rate
        // 在 modifiers 后取得 compositor 重复配置。
        .lock()
        // 配置损坏时不得猜测是否需要去重。
        .map_err(|_| "Wayland keyboard Key Press repeat-rate mutex poisoned")?;
    // 第五把锁固定为物理按键集合 owner。
    let keys_down = keys_down
        // 在 repeat-rate 后取得当前按下集合。
        .lock()
        // 集合损坏时不得猜测重复状态。
        .map_err(|_| "Wayland keyboard Key Press keys-down mutex poisoned")?;
    // 第六把锁固定为输入 serial owner。
    let input_serial = input_serial
        // 沿用 events→serial 的跨输入全局顺序。
        .lock()
        // serial 损坏时不得只投递按键事件。
        .map_err(|_| "Wayland keyboard Key Press input serial mutex poisoned")?;
    // 第七把锁固定为客户端重复候选 owner。
    let held_key = held_key_info
        // 在 serial owner 后取得候选槽。
        .lock()
        // 候选损坏时不得留下半次物理按下。
        .map_err(|_| "Wayland keyboard Key Press held-key mutex poisoned")?;
    // 第八把锁固定为重复节拍 owner。
    let last_repeat = last_repeat_time
        // 最后取得与 repeat commit 共用的节拍状态。
        .lock()
        // 节拍损坏时不得提交前七类 owner。
        .map_err(|_| "Wayland keyboard Key Press last-repeat mutex poisoned")?;
    // 全部 owners 健康后返回唯一事务 guards。
    Ok(Some((
        // 目标窗口来自同一 surface guard 快照。
        window_id,
        // guards 保持到完整 Press 提交结束。
        KeyboardKeyPressGuards {
            // 保留 surface 路由锁。
            _targets: targets,
            // 保留事件队列锁。
            events,
            // 保留修饰快照锁。
            modifiers,
            // 保留重复速率锁。
            repeat_rate,
            // 保留物理按键集合锁。
            keys_down,
            // 保留输入 serial 锁。
            input_serial,
            // 保留重复候选锁。
            held_key,
            // 保留重复节拍锁。
            last_repeat,
        },
    )))
}

// 按全局顺序取得 Release 所需的六类 owners。
fn lock_release_owners<'a>(
    // surface targets 是 Release 锁序的第一 owner。
    surface_windows: &'a Arc<Mutex<SurfaceWindowTargets>>,
    // event queue 是第二 owner。
    events: &'a Arc<Mutex<VecDeque<UiEvent>>>,
    // modifiers 是第三 owner。
    modifiers: &'a Arc<Mutex<KeyMod>>,
    // keys-down 是第四 owner。
    keys_down: &'a Arc<Mutex<HashSet<KeyCode>>>,
    // held-key 是第五 owner。
    held_key_info: &'a Arc<Mutex<Option<HeldKeyInfo>>>,
    // last-repeat 是第六 owner。
    last_repeat_time: &'a Arc<Mutex<Option<Instant>>>,
    // 无目标窗口返回 Ok(None)，状态失败返回稳定诊断。
) -> std::result::Result<Option<(WindowId, KeyboardKeyReleaseGuards<'a>)>, &'static str> {
    // 第一把锁固定为 surface 路由 owner。
    let targets = surface_windows
        // 取得不可恢复的健康 guard。
        .lock()
        // 路由损坏时不得推测目标窗口。
        .map_err(|_| "Wayland keyboard Key Release surface targets mutex poisoned")?;
    // 无 keyboard focus 的 Release 不能定向，保持所有 owner 不变。
    let Some(window_id) = targets.keyboard_target() else {
        // 无目标是可安全丢弃的协议事件，不形成 failure。
        return Ok(None);
    };
    // 第二把锁固定为 UI 事件队列 owner。
    let events = events
        // 在 surface guard 后取得队列。
        .lock()
        // 队列损坏时不得继续写入其他 owner。
        .map_err(|_| "Wayland keyboard Key Release event queue mutex poisoned")?;
    // 第三把锁固定为修饰快照 owner。
    let modifiers = modifiers
        // 在事件队列后取得真实修饰状态。
        .lock()
        // 禁止用 KeyMod::NONE 伪造损坏快照。
        .map_err(|_| "Wayland keyboard Key Release modifiers mutex poisoned")?;
    // 第四把锁固定为物理按键集合 owner。
    let keys_down = keys_down
        // 在 modifiers 后取得当前按下集合。
        .lock()
        // 集合损坏时不得生成伪 KeyUp。
        .map_err(|_| "Wayland keyboard Key Release keys-down mutex poisoned")?;
    // 第五把锁固定为客户端重复候选 owner。
    let held_key = held_key_info
        // 在 keys-down 后取得候选槽。
        .lock()
        // 候选损坏时不得只移除物理按键。
        .map_err(|_| "Wayland keyboard Key Release held-key mutex poisoned")?;
    // 第六把锁固定为重复节拍 owner。
    let last_repeat = last_repeat_time
        // 最后取得与 repeat commit 共用的节拍状态。
        .lock()
        // 节拍损坏时不得提交前五类 owner。
        .map_err(|_| "Wayland keyboard Key Release last-repeat mutex poisoned")?;
    // 全部 owners 健康后返回唯一事务 guards。
    Ok(Some((
        // 目标窗口来自同一 surface guard 快照。
        window_id,
        // guards 保持到完整 Release 提交结束。
        KeyboardKeyReleaseGuards {
            // 保留 surface 路由锁。
            _targets: targets,
            // 保留事件队列锁。
            events,
            // 保留修饰快照锁。
            modifiers,
            // 保留物理按键集合锁。
            keys_down,
            // 保留重复候选锁。
            held_key,
            // 保留重复节拍锁。
            last_repeat,
        },
    )))
}

// 在八类 owners 健康后一次提交 keyboard Key Press。
pub(crate) fn handle_keyboard_key_pressed(
    // compositor 为本次按下签发的 raw serial 只留在 Wayland 私有层。
    serial: u32,
    // seat adapter 已把 Linux 键码转换为平台中立键码。
    code: KeyCode,
    // surface targets 提供按下时的精确 keyboard focus。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // UI 事件队列接收同一事务中的 KeyDown 与 TextInput。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // modifiers 提供按下时的真实修饰快照。
    modifiers: &Arc<Mutex<KeyMod>>,
    // repeat-rate 决定是否由客户端去重 compositor 重复。
    repeat_rate: &Arc<Mutex<i32>>,
    // keys-down 保存物理按键集合。
    keys_down: &Arc<Mutex<HashSet<KeyCode>>>,
    // input serial 只在成功物理按下时推进。
    input_serial: &Arc<Mutex<InputSerial>>,
    // held-key 保存客户端重复候选。
    held_key_info: &Arc<Mutex<Option<HeldKeyInfo>>>,
    // last-repeat 在新物理按下时清空。
    last_repeat_time: &Arc<Mutex<Option<Instant>>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全丢弃当前协议事件。
) {
    // 沿统一顺序取得全部 Press owners。
    let owners = match lock_press_owners(
        // 第一 owner 是 surface 路由。
        surface_windows,
        // 第二 owner 是事件队列。
        events,
        // 第三 owner 是修饰快照。
        modifiers,
        // 第四 owner 是重复速率。
        repeat_rate,
        // 第五 owner 是物理按键集合。
        keys_down,
        // 第六 owner 是输入 serial。
        input_serial,
        // 第七 owner 是重复候选。
        held_key_info,
        // 第八 owner 是重复节拍。
        last_repeat_time,
    ) {
        // 健康 owners 进入目标判定。
        Ok(owners) => owners,
        // 任一 owner failure 不产生部分业务状态。
        Err(message) => {
            // guards 已由 Result 返回前自动释放。
            enqueue_owner_failure(pending_failures, message);
            // 安全丢弃当前 Press。
            return;
        }
    };
    // 无目标窗口时不记录 serial、不发布 UI 事件。
    let Some((window_id, mut owners)) = owners else {
        // 保持所有输入 owner 原状。
        return;
    };
    // 只在客户端重复启用且同键已按下时去重 compositor 重复。
    if *owners.repeat_rate > 0 && owners.keys_down.contains(&code) {
        // 去重分支在全部 guards 健康时仍保持零业务写入。
        return;
    }
    // 从同一 modifiers guard 复制平台中立修饰快照。
    let current_modifiers = *owners.modifiers;
    // 文本转换只读取同一按下快照中的 Shift 状态。
    let shift_down = current_modifiers.intersects(KeyMod::SHIFT);
    // 新候选的首按时刻只在所有 owners 健康且未去重后采集。
    let first_press = Instant::now();
    // 第一项业务提交记录本次真实协议 serial。
    owners.input_serial.record(serial);
    // 同一事务登记物理键已按下。
    owners.keys_down.insert(code);
    // KeyDown 是目标窗口收到的首个 UI 事件。
    owners.events.push_back(
        // 复用平台中立 KeyDown 构造器。
        UiEvent::key_down(code, current_modifiers)
            // 保持按下时的精确窗口路由。
            .for_window(window_id),
    );
    // 可打印 ASCII 继续生成与物理键事件互补的文本输入。
    if let Some(text) = keycode_to_char(code, shift_down) {
        // TextInput 必须紧随对应 KeyDown。
        owners.events.push_back(
            // 复用平台中立文本事件构造器。
            UiEvent::text_input(text)
                // 文本与物理键定向到同一窗口。
                .for_window(window_id),
        );
    }
    // 同一事务登记客户端重复候选快照。
    *owners.held_key = Some(HeldKeyInfo {
        // 保存平台中立键码。
        code,
        // 保存按下时修饰快照。
        mods: current_modifiers,
        // 保存未被锁等待提前污染的首按时刻。
        first_press,
        // 保存按下时精确窗口身份。
        window_id,
    });
    // 新物理按下重新开始客户端重复节拍。
    *owners.last_repeat = None;
}

// 在六类 owners 健康后一次提交 keyboard Key Release。
pub(crate) fn handle_keyboard_key_released(
    // seat adapter 已把 Linux 键码转换为平台中立键码。
    code: KeyCode,
    // surface targets 提供抬起时的精确 keyboard focus。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // UI 事件队列接收同一事务中的 KeyUp。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // modifiers 提供抬起时的真实修饰快照。
    modifiers: &Arc<Mutex<KeyMod>>,
    // keys-down 保存物理按键集合。
    keys_down: &Arc<Mutex<HashSet<KeyCode>>>,
    // held-key 在物理抬起时清空。
    held_key_info: &Arc<Mutex<Option<HeldKeyInfo>>>,
    // last-repeat 在物理抬起时清空。
    last_repeat_time: &Arc<Mutex<Option<Instant>>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全丢弃当前协议事件。
) {
    // 沿统一顺序取得全部 Release owners。
    let owners = match lock_release_owners(
        // 第一 owner 是 surface 路由。
        surface_windows,
        // 第二 owner 是事件队列。
        events,
        // 第三 owner 是修饰快照。
        modifiers,
        // 第四 owner 是物理按键集合。
        keys_down,
        // 第五 owner 是重复候选。
        held_key_info,
        // 第六 owner 是重复节拍。
        last_repeat_time,
    ) {
        // 健康 owners 进入目标判定。
        Ok(owners) => owners,
        // 任一 owner failure 不产生部分业务状态。
        Err(message) => {
            // guards 已由 Result 返回前自动释放。
            enqueue_owner_failure(pending_failures, message);
            // 安全丢弃当前 Release。
            return;
        }
    };
    // 无目标窗口时不移除按键、不发布 UI 事件。
    let Some((window_id, mut owners)) = owners else {
        // 保持所有输入 owner 原状。
        return;
    };
    // 从同一 modifiers guard 复制平台中立修饰快照。
    let current_modifiers = *owners.modifiers;
    // 第一项业务提交移除已抬起的物理键。
    owners.keys_down.remove(&code);
    // 同一事务把 KeyUp 定向到抬起时的焦点窗口。
    owners.events.push_back(
        // 复用平台中立 KeyUp 构造器。
        UiEvent::key_up(code, current_modifiers)
            // 保持抬起时的精确窗口路由。
            .for_window(window_id),
    );
    // 保留既有任一物理抬起都会停止单键重复的语义。
    *owners.held_key = None;
    // 同时清除上次客户端重复节拍。
    *owners.last_repeat = None;
}
