// Wayland keyboard repeat config Component 只协调共享状态，不持有协议对象。

// 两份配置通过短时互斥共享。
use std::sync::{Arc, Mutex};

// typed failure 保留 owner 与 RepeatInfo 阶段。
use crate::core::{Errc, Error};
// callback failure 进入 backend 已有 pending source。
use crate::diagnostics::PendingFailureSource;

// 将 RepeatInfo owner failure 投递到同一 backend source。
fn enqueue_owner_failure(
    // 每个 keyboard callback 捕获同一 pending source。
    pending_failures: &PendingFailureSource,
    // 诊断文本由具体配置 owner 固定提供。
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

// 在两份 owners 健康后一次提交 keyboard RepeatInfo 配置。
pub(crate) fn handle_keyboard_repeat_info(
    // compositor 提供的原始有符号重复速率。
    rate: i32,
    // compositor 提供的原始有符号启动延迟。
    delay: i32,
    // repeat-rate 是配置事务的第一 owner。
    repeat_rate: &Arc<Mutex<i32>>,
    // repeat-delay 是配置事务的第二 owner。
    repeat_delay: &Arc<Mutex<i32>>,
    // 任一 owner failure 进入 backend pending source。
    pending_failures: &PendingFailureSource,
    // 端口不返回可恢复值，失败时安全保留原配置。
) {
    // 第一把锁固定为 repeat-rate owner。
    let mut current_rate = match repeat_rate.lock() {
        // 健康 guard 保持到双配置提交完成。
        Ok(current_rate) => current_rate,
        // rate 损坏时不得只更新 delay。
        Err(_) => {
            // 投递可定位的 rate owner 失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 RepeatInfo 与 rate owner 阶段。
                "Wayland keyboard RepeatInfo repeat-rate mutex poisoned",
            );
            // 两份配置保持原状。
            return;
        }
    };
    // 第二把锁固定为 repeat-delay owner。
    let mut current_delay = match repeat_delay.lock() {
        // 健康 guard 允许开始一次性提交。
        Ok(current_delay) => current_delay,
        // delay 损坏时不得先提交新 rate。
        Err(_) => {
            // 投递可定位的 delay owner 失败。
            enqueue_owner_failure(
                // 使用同一 backend source。
                pending_failures,
                // 保留 RepeatInfo 与 delay owner 阶段。
                "Wayland keyboard RepeatInfo repeat-delay mutex poisoned",
            );
            // rate guard 尚未修改状态。
            return;
        }
    };
    // 两把 guards 健康后提交 compositor 原始 rate。
    *current_rate = rate;
    // 同一事务提交配套的 compositor 原始 delay。
    *current_delay = delay;
}
