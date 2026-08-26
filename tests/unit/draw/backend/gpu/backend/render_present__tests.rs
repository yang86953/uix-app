// 引入待验证的 typed submission 门禁。
use super::require_lossless_main_surface_submission;
// 引入错误分类用于稳定断言。
use crate::core::Errc;

// 完整提交应保持成功，未覆盖提交必须返回 NotImplemented。
#[test]
fn main_surface_submission_requires_lossless_rhi() {
    // 成功 lowering 不应制造额外状态错误。
    assert!(require_lossless_main_surface_submission(true, "unused").is_ok());
    // 模拟所有 RHI 路径均无法无损覆盖当前队列。
    let failure = require_lossless_main_surface_submission(false, "missing main RHI lowering");
    // 恢复层必须能稳定识别覆盖缺口，而不是收到伪成功。
    assert!(matches!(
        // 检查 helper 返回的 typed error。
        failure,
        // 禁止把未覆盖队列归类为参数或平台故障。
        Err(error) if error.code() == Errc::NotImplemented
    ));
}
