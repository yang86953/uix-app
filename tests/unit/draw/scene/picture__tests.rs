// 引入待验证的 Picture 资源协调函数。
use super::ensure_offscreen;
// 引入稳定 OOM 错误分类。
use crate::core::Errc;
// 使用 API-neutral recorder 触发确定性的 extent failure。
use crate::draw::painting::recorder::CommandRecorder;

// 验证场景不会把 typed Picture 创建失败误作普通缓存不可用。
#[test]
// 锁定 ensure_offscreen 的 Result 传播边界。
fn ensure_offscreen_propagates_typed_allocation_failure() {
    // 创建不依赖原生窗口的 recorder target。
    let mut recorder = CommandRecorder::new();
    // 初始状态没有已发布的 Picture handle。
    let mut handle = None;
    // 极大 extent 必须触发 recorder 的 typed OOM。
    let result = ensure_offscreen(
        // 通过 RenderTarget 契约调用 recorder。
        &mut recorder,
        // 允许函数在成功时发布新 handle。
        &mut handle,
        // 使用无法满足像素预算的逻辑宽度。
        i32::MAX,
        // 使用无法满足像素预算的逻辑高度。
        i32::MAX,
    );
    // 场景层必须保留原始 OOM 分类供恢复驱动消费。
    assert!(matches!(
        // 匹配完整检查式结果。
        result,
        // 禁止将失败折叠为 Ok(false)。
        Err(error) if error.code() == Errc::GraphicsOutOfMemory
    ));
    // 失败事务不得发布半成品 Picture handle。
    assert!(handle.is_none());
}
