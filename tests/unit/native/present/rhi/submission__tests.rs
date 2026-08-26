// 引入被测共享提交序列。
use super::RhiSubmissionSequence;
// 引入构造零值和迟到提交所需的句柄类型。
use crate::platform::presentation::rhi::SubmissionHandle;

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
