//! Platform System 组合输入的中立所有权合同。

use crate::diagnostics::PendingFailureQueue;

/// 保存尚未交付给原生平台聚合的运行时选项。
///
/// 该值按所有权传入平台组合根；故障队列只能随本值被消费一次，调用方无法
/// 在平台对象构造后再次取回或替换同一组启动输入。
pub(crate) struct PendingNativeOptions {
    pending_failures: PendingFailureQueue,
}

impl PendingNativeOptions {
    /// 创建等待由唯一平台组合根消费的启动选项。
    pub(crate) fn new(pending_failures: PendingFailureQueue) -> Self {
        Self { pending_failures }
    }

    /// 将完整启动输入一次性交给组合根。
    pub(super) fn into_pending_failures(self) -> PendingFailureQueue {
        self.pending_failures
    }
}
