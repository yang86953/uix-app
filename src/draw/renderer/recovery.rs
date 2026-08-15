//! 有界图形恢复状态机。
//!
//! 恢复动作在帧边界依据类型化 [`GraphicsFailure`] 决定。状态机不会在帧中执行热探测：
//! 调用方负责执行返回的动作，并把下一次失败重新报告给状态机。

use super::GraphicsFailure;

/// 图形帧失败后允许执行的下一个恢复动作。
///
/// 与 `RecoveryAction`（错误恢复处理器决定，旧 `crate::diagnostics::recovery` 路径已迁移）
/// 同名异构，故图形侧显式命名为 GraphicsRecoveryAction 以示区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsRecoveryAction {
    /// 使用当前 recipe 重建呈现 surface。
    RebuildSurface,
    /// 重新创建当前完整图形 recipe。
    RebuildRecipe,
    /// 尝试配置序列中的下一条图形 recipe。
    TryNextRecipe,
    /// 切换到唯一一次软件渲染恢复尝试。
    UseSoftware,
    /// 因图形内存不足进入不可恢复终态。
    AbortOutOfMemory,
    /// 因恢复序列耗尽进入终态。
    Abort,
}

/// 每个窗口独立持有的有界恢复进度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsRecovery {
    step: u8,
    software_available: bool,
}

impl GraphicsRecovery {
    /// 创建尚未尝试恢复动作的窗口恢复状态。
    pub const fn new(software_available: bool) -> Self {
        Self {
            step: 0,
            software_available,
        }
    }

    /// 将 typed failure 映射到唯一的有界恢复序列：
    /// surface rebuild -> same recipe rebuild -> next recipe -> Software。
    /// Software 只尝试一次；OOM 与序列耗尽均进入终态。
    pub fn on_failure(&mut self, failure: &GraphicsFailure) -> GraphicsRecoveryAction {
        if matches!(failure, GraphicsFailure::OutOfMemory(_)) {
            return GraphicsRecoveryAction::AbortOutOfMemory;
        }
        let action = match self.step {
            0 => GraphicsRecoveryAction::RebuildSurface,
            1 => GraphicsRecoveryAction::RebuildRecipe,
            2 => GraphicsRecoveryAction::TryNextRecipe,
            3 if self.software_available => GraphicsRecoveryAction::UseSoftware,
            _ => GraphicsRecoveryAction::Abort,
        };
        self.step = self.step.saturating_add(1);
        action
    }

    // 将已证实不兼容的硬件呈现链直接推进到 Software，避免换用另一条仍不可见的 GPU swapchain。
    pub(crate) fn prefer_software(&mut self) {
        // 只有存在 Software 候选时才改变有界恢复游标。
        if self.software_available {
            // 第三号步骤对应唯一一次 UseSoftware 动作。
            self.step = 3;
        }
    }

    /// 在成功呈现后结束当前失败周期，并为下一次独立失败重置有界序列。
    pub fn on_presented(&mut self) {
        self.step = 0;
    }
}
