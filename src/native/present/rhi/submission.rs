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

    // 使当前提交身份失效，供 context 释放或重建时切断迟到呈现。
    pub(crate) fn invalidate(&mut self) {
        // 清除最近提交，但保留单调序号以免复用旧身份。
        self.last = None;
    }
}

// 仅验证 API 无关提交状态机的边界语义。
#[cfg(test)]
mod tests {
    // 引入被测共享提交序列。
    use super::RhiSubmissionSequence;
    // 引入构造零值和迟到提交所需的句柄类型。
    use crate::native::present::rhi::SubmissionHandle;

    // 验证初始状态和零值都不能进入 Surface present。
    #[test]
    fn sequence_rejects_handles_before_first_issue() {
        // 创建尚未签发任何提交的序列。
        let sequence = RhiSubmissionSequence::new();
        // 初始状态必须拒绝零提交身份。
        assert!(!sequence.is_latest(SubmissionHandle::from_raw(0)));
        // 初始状态也必须拒绝尚未签发的非零身份。
        assert!(!sequence.is_latest(SubmissionHandle::from_raw(1)));
    }

    // 验证新提交会替换旧提交，两个 Adapter 共享同一最新值语义。
    #[test]
    fn sequence_only_accepts_latest_issued_handle() {
        // 创建共享提交序列。
        let mut sequence = RhiSubmissionSequence::new();
        // 签发第一份提交身份。
        let first = sequence.issue().expect("first submission should be issued");
        // 第一份提交在下一次签发前必须有效。
        assert!(sequence.is_latest(first));
        // 签发第二份提交身份。
        let second = sequence
            .issue()
            .expect("second submission should be issued");
        // 新提交必须让旧提交立即失效。
        assert!(!sequence.is_latest(first));
        // 第二份提交必须成为唯一最新身份。
        assert!(sequence.is_latest(second));
    }

    // 验证 context 释放会切断最后一个仍可见的提交身份。
    #[test]
    fn sequence_invalidation_rejects_last_handle() {
        // 创建并签发一份提交身份。
        let mut sequence = RhiSubmissionSequence::new();
        // 保存释放前的最新提交。
        let submission = sequence.issue().expect("submission should be issued");
        // 模拟 context 资源释放或重建。
        sequence.invalidate();
        // 释放后的迟到 present 必须被拒绝。
        assert!(!sequence.is_latest(submission));
    }

    // 验证序号耗尽后不会复用句柄或伪造一次成功提交。
    #[test]
    fn sequence_reports_exhaustion_without_reusing_identity() {
        // 构造只剩最后一个非零身份的边界状态。
        let mut sequence = RhiSubmissionSequence {
            // 下一次签发 u64 最大值。
            next_raw: Some(u64::MAX),
            // 边界测试开始时没有最近提交。
            last: None,
        };
        // 最后一个合法身份仍应成功签发。
        let last = sequence.issue().expect("last submission should be issued");
        // 签发结果必须保持最大值身份。
        assert_eq!(last.raw(), u64::MAX);
        // 序列耗尽后必须返回显式错误。
        assert!(sequence.issue().is_err());
        // 耗尽失败不得篡改最后一次成功提交。
        assert!(sequence.is_latest(last));
    }
}
