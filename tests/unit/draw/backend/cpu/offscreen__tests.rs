// 引入当前模块的私有离屏池。
use super::CpuOffscreenPool;
// 引入稳定 OOM 错误分类。
use crate::core::Errc;

// 验证正常无资源与真实分配失败保持不同结果。
#[test]
// 锁定 CPU Picture 创建不会吞掉 typed OOM。
fn checked_create_distinguishes_invalid_extent_from_oom() {
    // 创建独立 CPU Picture 资源 owner。
    let mut pool = CpuOffscreenPool::new();
    // 非正尺寸是正常无资源，不进入恢复错误。
    let invalid = pool.try_create(0, 1);
    // 调用方应能继续无缓存路径。
    assert!(matches!(invalid, Ok(None)));

    // 极大尺寸必须在尝试真实分配前后返回 typed OOM。
    let failure = pool.try_create(i32::MAX, i32::MAX);
    // 禁止把 CPU 像素分配失败重新降成 None。
    assert!(matches!(
        // 匹配检查式创建的完整结果。
        failure,
        // 统一分类必须进入 graphics recovery 的 OOM 分支。
        Err(error) if error.code() == Errc::GraphicsOutOfMemory
    ));
}
