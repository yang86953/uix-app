// Wayland keyboard modifier owner Component 只协调共享状态，不持有协议对象。

// 多个 callback 通过短时互斥共享 owner 状态。
use std::sync::{Arc, Mutex};
// 重复节拍 owner 保存单调时刻。
use std::time::Instant;

// typed failure 保留 owner 与 Modifiers 阶段。
use crate::core::{Errc, Error};
// callback failure 进入 backend 已有 pending source。
use crate::diagnostics::PendingFailureSource;
// 平台中立 KeyMod 是修饰键状态的唯一共享表示。
use crate::platform::windowing::KeyMod;

// held-key owner 继续使用 Wayland 后端私有值类型。
use super::HeldKeyInfo;

// 将 Modifiers callback owner failure 投递到同一 backend source。
fn enqueue_owner_failure(
    // 每个 keyboard callback 捕获同一 pending source。
    pending_failures: &PendingFailureSource,
    // 诊断文本由具体 owner 固定提供。
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

// 把 Wayland xkb 修饰位图映射为平台中立 KeyMod。
fn key_mod_from_wayland_bits(
    // depressed、latched 与 locked 已由调用方合并。
    combined: u32,
    // 纯函数不访问任何共享 owner。
) -> KeyMod {
    // 从没有修饰键的状态开始构造。
    let mut modifiers = KeyMod::NONE;
    // xkb 位一表示 Shift。
    if combined & 1 != 0 {
        // 合并平台中立 Shift 标志。
        modifiers |= KeyMod::SHIFT;
    }
    // xkb 位四表示 Control。
    if combined & 4 != 0 {
        // 合并平台中立 Ctrl 标志。
        modifiers |= KeyMod::CTRL;
    }
    // xkb 位八表示 Alt。
    if combined & 8 != 0 {
        // 合并平台中立 Alt 标志。
        modifiers |= KeyMod::ALT;
    }
    // xkb 位十六表示 Super。
    if combined & 16 != 0 {
        // 合并平台中立 Super 标志。
        modifiers |= KeyMod::SUPER;
    }
    // 返回完整且唯一的修饰快照。
    modifiers
}

// 在三个 owners 健康后一次提交 keyboard Modifiers。
pub(crate) fn handle_keyboard_modifiers(
    // compositor 当前按下的修饰位图。
    mods_depressed: u32,
    // compositor 当前锁存的修饰位图。
    mods_latched: u32,
    // compositor 当前锁定的修饰位图。
    mods_locked: u32,
    // modifier state 是 Key callback 的共享快照 owner。
    modifier_state: &Arc<Mutex<KeyMod>>,
    // held-key 在修饰变化时必须失效。
    held_key_info: &Arc<Mutex<Option<HeldKeyInfo>>>,
    // last-repeat 在修饰变化时必须失效。
    last_repeat_time: &Arc<Mutex<Option<Instant>>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全保留原状态。
) {
    // 第一把锁固定为 modifier state owner。
    let mut modifiers = match modifier_state.lock() {
        // 健康 guard 保持到三份状态提交完成。
        Ok(modifiers) => modifiers,
        // 修饰状态损坏时不得只清除重复状态。
        Err(_) => {
            // 投递可定位的 modifier owner 失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Modifiers 与 modifier owner 阶段。
                "Wayland keyboard Modifiers modifier state mutex poisoned",
            );
            // 三份共享事实保持原状。
            return;
        }
    };
    // 第二把锁固定为 held-key owner。
    let mut held_key = match held_key_info.lock() {
        // 健康 guard 与 modifier guard 一起保留到提交完成。
        Ok(held_key) => held_key,
        // 重复候选损坏时不得先更新修饰快照。
        Err(_) => {
            // 投递可定位的 held-key owner 失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Modifiers 与 held-key owner 阶段。
                "Wayland keyboard Modifiers held-key mutex poisoned",
            );
            // modifier guard 尚未修改状态。
            return;
        }
    };
    // 第三把锁固定为 last-repeat owner。
    let mut last_repeat = match last_repeat_time.lock() {
        // 健康 guard 允许开始一次性提交。
        Ok(last_repeat) => last_repeat,
        // 重复节拍损坏时不得部分提交前两份状态。
        Err(_) => {
            // 投递可定位的 last-repeat owner 失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 Modifiers 与 last-repeat owner 阶段。
                "Wayland keyboard Modifiers last-repeat mutex poisoned",
            );
            // 前两份 guards 尚未修改状态。
            return;
        }
    };
    // 三类协议位图共同组成当前有效修饰集合。
    let combined = mods_depressed | mods_latched | mods_locked;
    // 所有 guards 健康后提交新的平台中立修饰快照。
    *modifiers = key_mod_from_wayland_bits(combined);
    // 同一事务使旧修饰下的重复候选失效。
    *held_key = None;
    // 同一事务清除旧修饰下的重复节拍。
    *last_repeat = None;
}
