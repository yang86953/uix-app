// 引入当前模块的统一报告入口。
use super::report_center_on_screen_result;
// 引入构造 typed failure 所需的错误类型与错误码。
use crate::core::{Errc, Error};

// 验证只有真实执行失败会进入警告路径。
#[test]
// 单一测试覆盖成功、预期缺失与真实失败三种完整分类。
fn suppresses_only_expected_not_implemented_failure() {
    // 成功结果不得记录警告。
    assert!(!report_center_on_screen_result("success", Ok(())));
    // 构造 Wayland 当前使用的预期能力缺失。
    let unsupported = Error::new(Errc::NotImplemented, "center is compositor-owned");
    // 预期能力缺失不得记录警告。
    assert!(!report_center_on_screen_result(
        "unsupported",
        Err(unsupported)
    ));
    // 构造必须继续暴露的真实平台执行失败。
    let failed = Error::new(Errc::PlatformError, "native center failed");
    // 真实失败必须经过警告路径。
    assert!(report_center_on_screen_result("failed", Err(failed)));
    // 结束自动居中分类测试。
}
// 结束自动居中结果测试模块。
