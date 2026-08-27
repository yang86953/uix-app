//! 图形 API 无关的 RHI 提交序列契约。
//!
//! Device 只能从这里签发非零提交身份，Surface 只能接受同一组合 context
//! 最近一次成功签发的身份；原生 Adapter 不得各自解释提交与呈现的关联。

// 引入统一错误码、错误值和结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入共享的不透明提交句柄。
use super::SubmissionHandle;

// 保存单一组合 context 的提交序列和最近一次成功提交。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiSubmissionSequence {
    // 保存下一次可签发的非零身份；None 表示序号空间已经耗尽。
    next_raw: Option<u64>,
    // 保存当前允许 Surface 接受的最近提交身份。
    last: Option<SubmissionHandle>,
}

// 为共享提交序列提供唯一状态机实现。
impl RhiSubmissionSequence {
    // 创建从一开始签发且尚无可呈现提交的序列。
    pub(crate) const fn new() -> Self {
        // 初始化非零序号并保持最近提交为空。
        Self {
            // 第一次成功提交固定获得身份一。
            next_raw: Some(1),
            // 构造阶段没有任何提交可以进入 Surface。
            last: None,
        }
    }

    // 签发一次新的提交身份并把它设为唯一最新提交。
    pub(crate) fn issue(&mut self) -> Result<SubmissionHandle> {
        // 序号空间耗尽时拒绝复用旧身份，避免迟到提交重新变成有效。
        let Some(raw) = self.next_raw else {
            // 返回稳定的设备状态错误。
            return Err(Error::new(
                // 序号耗尽表示当前 context 已不能继续建立可靠事务。
                Errc::InvalidState,
                // 错误文本不携带任何具体图形 API 名称。
                "RHI submission sequence is exhausted",
            ));
        };
        // 预先计算下一身份；u64 最大值签发后序列进入不可复用的耗尽态。
        self.next_raw = raw.checked_add(1);
        // 把非零数值封装为不透明提交身份。
        let submission = SubmissionHandle::from_raw(raw);
        // 只有本次成功签发的身份可以通过后续 Surface 校验。
        self.last = Some(submission);
        // 返回共享契约签发的提交身份。
        Ok(submission)
    }

    // 判断指定身份是否为当前组合 context 最近一次成功提交。
    pub(crate) fn is_latest(&self, submission: SubmissionHandle) -> bool {
        // 显式拒绝零值，并要求身份与共享状态完全相等。
        submission.raw() != 0 && self.last == Some(submission)
    }

    // 验证 Surface present 使用的是同一组合 context 的最新提交。
    pub(crate) fn validate(&self, submission: SubmissionHandle) -> Result<()> {
        // 零值、迟到值和其它 context 的身份都不能进入原生呈现。
        if !self.is_latest(submission) {
            // 返回跨后端稳定的调用契约错误。
            return Err(Error::new(
                // 外部或迟到身份不表示设备或 Surface 丢失。
                Errc::InvalidArgument,
                // 诊断文本不携带具体图形 API 名称。
                "RHI present submission is stale",
            ));
        }
        // 当前最新提交可以继续进入 Surface 门禁。
        Ok(())
    }

    // 使当前提交身份失效，供 context 释放或重建时切断迟到呈现。
    pub(crate) fn invalidate(&mut self) {
        // 清除最近提交，但保留单调序号以免复用旧身份。
        self.last = None;
    }
}
